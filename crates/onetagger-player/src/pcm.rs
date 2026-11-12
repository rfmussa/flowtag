use anyhow::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use rodio::Source;
use rodio::source::SeekError;

/// Fully decoded interleaved 16-bit PCM audio
pub struct DecodedPcm {
    pub channels: u16,
    pub sample_rate: u32,
    pub samples: Vec<i16>,
}

/// Seekable playback over shared decoded PCM
pub struct PcmPlayback {
    data: Arc<DecodedPcm>,
    index: usize,
}

impl PcmPlayback {
    pub fn new(data: Arc<DecodedPcm>) -> PcmPlayback {
        PcmPlayback { data, index: 0 }
    }
}

impl Iterator for PcmPlayback {
    type Item = i16;
    fn next(&mut self) -> Option<i16> {
        let sample = *self.data.samples.get(self.index)?;
        self.index += 1;
        Some(sample)
    }
}

impl Source for PcmPlayback {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { self.data.channels }
    fn sample_rate(&self) -> u32 { self.data.sample_rate }

    fn total_duration(&self) -> Option<Duration> {
        let channels = self.data.channels.max(1) as u64;
        let rate = self.data.sample_rate.max(1) as u64;
        let frames = self.data.samples.len() as u64 / channels;
        Some(Duration::from_millis(frames * 1000 / rate))
    }

    /// Seek in place; always succeeds, clamping past-EOF to the end
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        let millis = pos.as_millis().min(u64::MAX as u128) as u64;
        let frame = millis.saturating_mul(self.data.sample_rate as u64) / 1000;
        let index = frame.saturating_mul(self.data.channels as u64);
        self.index = index.min(self.data.samples.len() as u64) as usize;
        Ok(())
    }
}

struct CacheFillInner {
    cache: Mutex<Option<Arc<DecodedPcm>>>,
    /// Set while a background fill is decoding; at most one fill runs at a time
    filling: AtomicBool,
}

/// Decoded-PCM cache filled once in the background. Cloning shares the same cache,
/// so a fill outlives the handle that started it and lands for every other clone.
#[derive(Clone)]
pub struct CacheFill(Arc<CacheFillInner>);

impl CacheFill {
    pub fn new() -> CacheFill {
        CacheFill(Arc::new(CacheFillInner { cache: Mutex::new(None), filling: AtomicBool::new(false) }))
    }

    fn guard(&self) -> MutexGuard<'_, Option<Arc<DecodedPcm>>> {
        self.0.cache.lock().unwrap_or_else(|e| {
            warn!("PCM cache lock was poisoned, recovering");
            e.into_inner()
        })
    }

    /// The decoded audio, once a fill has landed
    pub fn get(&self) -> Option<Arc<DecodedPcm>> {
        self.guard().clone()
    }

    /// Start `decode` on a background thread unless the cache is already filled or a
    /// fill is running. On success the result is cached; on error it is logged and
    /// nothing is cached, so a later call can retry. Returns the thread when one started.
    pub fn spawn_fill<F>(&self, decode: F) -> Option<JoinHandle<()>>
    where F: FnOnce() -> Result<DecodedPcm, Error> + Send + 'static {
        if self.get().is_some() {
            return None;
        }
        if self.0.filling.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return None;
        }
        // A fill may have landed between the cache check and claiming the flag
        if self.get().is_some() {
            self.0.filling.store(false, Ordering::SeqCst);
            return None;
        }
        let fill = self.clone();
        let spawned = thread::Builder::new().name("pcm-cache-fill".into()).spawn(move || {
            match decode() {
                Ok(pcm) => *fill.guard() = Some(Arc::new(pcm)),
                Err(e) => warn!("PCM cache fill failed: {e}"),
            }
            fill.0.filling.store(false, Ordering::SeqCst);
        });
        match spawned {
            Ok(handle) => Some(handle),
            Err(e) => {
                warn!("PCM cache fill could not start: {e}");
                self.0.filling.store(false, Ordering::SeqCst);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcm() -> Arc<DecodedPcm> {
        // 2ch, 1000Hz, 8 frames: frame f -> [f*10, f*10+1]
        let samples = (0..8i16).flat_map(|f| [f * 10, f * 10 + 1]).collect();
        Arc::new(DecodedPcm { channels: 2, sample_rate: 1000, samples })
    }

    #[test]
    fn try_seek_lands_on_exact_frame() {
        let mut p = PcmPlayback::new(pcm());
        p.try_seek(Duration::from_millis(3)).unwrap();
        assert_eq!(p.next(), Some(30));
        assert_eq!(p.next(), Some(31));
    }

    #[test]
    fn try_seek_past_eof_clamps_to_end() {
        let mut p = PcmPlayback::new(pcm());
        p.try_seek(Duration::from_secs(10)).unwrap();
        assert_eq!(p.next(), None);
    }

    fn tiny() -> DecodedPcm {
        DecodedPcm { channels: 1, sample_rate: 1000, samples: vec![1, 2, 3] }
    }

    #[test]
    fn fill_runs_once_concurrently() {
        let fill = CacheFill::new();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let first = fill.spawn_fill(move || {
            release_rx.recv().ok();
            Ok(tiny())
        });
        assert!(first.is_some(), "first fill must start");
        // While the first fill is still decoding, a second request is a no-op
        assert!(fill.spawn_fill(|| panic!("second fill must not run")).is_none());
        assert!(fill.get().is_none(), "cache stays empty until the fill lands");
        release_tx.send(()).unwrap();
        first.unwrap().join().unwrap();
        assert_eq!(fill.get().map(|p| p.samples.clone()), Some(vec![1, 2, 3]));
        // Once cached, no further fill starts
        assert!(fill.spawn_fill(|| panic!("cached: fill must not run")).is_none());
    }

    #[test]
    fn failed_fill_clears_flag_without_caching() {
        let fill = CacheFill::new();
        fill.spawn_fill(|| Err(anyhow::anyhow!("boom"))).unwrap().join().unwrap();
        assert!(fill.get().is_none());
        // The flag was cleared, so a retry can start and succeed
        let retry = fill.spawn_fill(|| Ok(tiny()));
        assert!(retry.is_some());
        retry.unwrap().join().unwrap();
        assert!(fill.get().is_some());
    }

    #[test]
    fn source_properties_match() {
        let p = PcmPlayback::new(pcm());
        assert_eq!(p.channels(), 2);
        assert_eq!(p.sample_rate(), 1000);
        assert_eq!(p.total_duration(), Some(Duration::from_millis(8)));
        assert_eq!(p.current_frame_len(), None);
    }
}

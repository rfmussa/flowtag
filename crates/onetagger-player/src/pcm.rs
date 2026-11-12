use std::sync::Arc;
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
        let frame = pos.as_millis() as u64 * self.data.sample_rate as u64 / 1000;
        let index = frame.saturating_mul(self.data.channels as u64);
        self.index = index.min(self.data.samples.len() as u64) as usize;
        Ok(())
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

    #[test]
    fn source_properties_match() {
        let p = PcmPlayback::new(pcm());
        assert_eq!(p.channels(), 2);
        assert_eq!(p.sample_rate(), 1000);
        assert_eq!(p.total_duration(), Some(Duration::from_millis(8)));
        assert_eq!(p.current_frame_len(), None);
    }
}

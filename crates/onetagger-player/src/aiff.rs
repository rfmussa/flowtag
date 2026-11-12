use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use anyhow::Error;
use lofty::file::AudioFile;
use std::fs::File;
use std::io::Read;
use std::time::Duration;
use pacmog::PcmReader;
use rodio::Source;

use crate::AudioSource;
use crate::pcm::{DecodedPcm, PcmPlayback};

pub struct AIFFSource {
    path: PathBuf,
    duration: Duration,
    /// Decoded audio, filled on first `get_source` and reused for seeks / replays
    cache: Mutex<Option<Arc<DecodedPcm>>>,
}

impl AIFFSource {
    // Load from path
    pub fn new(path: impl AsRef<Path>) -> Result<AIFFSource, Error> { 
        // Get duration
        let file = lofty::read_from_path(&path)?;
        let duration = file.properties().duration();

        Ok(AIFFSource {
            path: path.as_ref().to_owned(),
            duration,
            cache: Mutex::new(None),
        })
    }

    #[cfg(test)]
    pub(crate) fn cached(&self) -> Option<Arc<DecodedPcm>> {
        self.cache.lock().ok()?.clone()
    }
}

impl AudioSource for AIFFSource {
    // Get duration
    fn duration(&self) -> u128 {
        self.duration.as_millis()

    }

    // Get rodio source
    fn get_source(&self) -> Result<Box<dyn Source<Item = i16> + Send>, Error> {
        let mut cache = self.cache.lock().map_err(|_| anyhow!("AIFF cache poisoned"))?;
        let pcm = match cache.as_ref() {
            Some(pcm) => pcm.clone(),
            None => {
                let mut data = vec![];
                File::open(&self.path)?.read_to_end(&mut data)?;
                let pcm = Arc::new(decode_aiff(&data)?);
                *cache = Some(pcm.clone());
                pcm
            }
        };
        Ok(Box::new(PcmPlayback::new(pcm)))
    }
}

/// Decode a whole AIFF file into interleaved i16 PCM.
/// pacmog panics on malformed data, so parsing and reading run inside one catch_unwind.
fn decode_aiff(data: &[u8]) -> Result<DecodedPcm, Error> {
    // TODO: Make better using symphonia / new rodio
    std::panic::catch_unwind(|| -> Result<DecodedPcm, Error> {
        let reader = PcmReader::new(&mut &data[..]).map_err(|e| anyhow!("Not an AIFF file: {e}"))?;
        let specs = reader.get_pcm_specs();
        let mut samples = Vec::with_capacity(specs.num_channels as usize * specs.num_samples as usize);
        for sample in 0..specs.num_samples {
            for channel in 0..specs.num_channels {
                let s = reader.read_sample::<f32>(channel, sample)
                    .map_err(|e| anyhow!("Failed decoding AIFF: {e}"))?;
                samples.push((s * i16::MAX as f32) as i16);
            }
        }
        Ok(DecodedPcm {
            channels: specs.num_channels,
            sample_rate: specs.sample_rate,
            samples,
        })
    }).map_err(|e| anyhow!("Failed decoding AIFF: {e:?}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal big-endian AIFF: 2ch, 16-bit, 8000Hz, sample = frame*10+channel
    fn tiny_aiff(frames: u16) -> Vec<u8> {
        let mut comm = vec![];
        comm.extend(2u16.to_be_bytes());
        comm.extend((frames as u32).to_be_bytes());
        comm.extend(16u16.to_be_bytes());
        comm.extend([0x40, 0x0B, 0xFA, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let mut ssnd = vec![0u8; 8]; // offset + blockSize
        for f in 0..frames {
            for c in 0..2u16 {
                ssnd.extend(((f * 10 + c) as i16).to_be_bytes());
            }
        }
        let mut body = b"AIFF".to_vec();
        body.extend(b"COMM");
        body.extend((comm.len() as u32).to_be_bytes());
        body.extend(&comm);
        body.extend(b"SSND");
        body.extend((ssnd.len() as u32).to_be_bytes());
        body.extend(&ssnd);
        let mut out = b"FORM".to_vec();
        out.extend((body.len() as u32).to_be_bytes());
        out.extend(body);
        out
    }

    #[test]
    fn bulk_decode_matches_samples() {
        let pcm = decode_aiff(&tiny_aiff(20)).unwrap();
        assert_eq!(pcm.channels, 2);
        assert_eq!(pcm.sample_rate, 8000);
        assert_eq!(pcm.samples.len(), 40);
        for f in 0..20usize {
            for c in 0..2usize {
                let want = (f * 10 + c) as i16;
                let got = pcm.samples[f * 2 + c];
                // f32 round trip may be off by one LSB
                assert!((got as i32 - want as i32).abs() <= 1, "frame {f} ch {c}: {got} vs {want}");
            }
        }
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(decode_aiff(b"definitely not an aiff file").is_err());
    }

    #[test]
    fn get_source_caches_decode() {
        let path = std::env::temp_dir().join(format!("flowtag_tiny_{}.aiff", std::process::id()));
        std::fs::write(&path, tiny_aiff(20)).unwrap();
        let src = AIFFSource::new(&path).unwrap();
        assert!(src.cached().is_none());
        src.get_source().unwrap();
        let first = src.cached().unwrap();
        src.get_source().unwrap();
        let second = src.cached().unwrap();
        std::fs::remove_file(&path).ok();
        assert!(Arc::ptr_eq(&first, &second));
    }
}

use anyhow::Error;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::BufReader;
use rodio::Source;
use alac::{Reader, StreamInfo};

use crate::pcm::{DecodedPcm, PcmPlayback};

/// ALAC stream inside an MP4 container. Duration is reported by `MP4Source`.
pub struct ALACSource {
    path: PathBuf,
    stream_info: StreamInfo,
    /// Decoded audio, filled on first `get_source` and reused for seeks / replays
    cache: Mutex<Option<Arc<DecodedPcm>>>,
}

impl ALACSource {
    // Read stream info from file
    pub fn new(path: impl AsRef<Path>) -> Result<ALACSource, Error> {
        let file = File::open(&path)?;
        let reader = Reader::new(BufReader::new(file))?;
        Ok(ALACSource {
            path: path.as_ref().to_owned(),
            stream_info: reader.stream_info().to_owned(),
            cache: Mutex::new(None),
        })
    }

    // Get rodio source, decoding the whole stream on first call
    pub fn get_source(&self) -> Result<Box<dyn Source<Item = i16> + Send>, Error> {
        let mut cache = self.cache.lock().map_err(|_| anyhow!("ALAC cache poisoned"))?;
        let pcm = match cache.as_ref() {
            Some(pcm) => pcm.clone(),
            None => {
                let pcm = Arc::new(self.decode()?);
                *cache = Some(pcm.clone());
                pcm
            }
        };
        Ok(Box::new(PcmPlayback::new(pcm)))
    }

    /// Decode the whole stream into interleaved i16 PCM. Errors are not cached.
    fn decode(&self) -> Result<DecodedPcm, Error> {
        let reader = Reader::new(BufReader::new(File::open(&self.path)?))?;
        let mut samples = Vec::new();
        for s in reader.into_samples::<i32>() {
            samples.push(alac_sample_to_i16(s.map_err(|e| anyhow!("Failed decoding ALAC: {e:?}"))?));
        }
        Ok(DecodedPcm {
            channels: self.stream_info.channels() as u16,
            sample_rate: self.stream_info.sample_rate(),
            samples,
        })
    }
}

/// Convert a decoded ALAC sample to 16-bit.
/// The alac crate left-aligns samples of every bit depth to the full 32 bits, so always shift down 16.
pub(crate) fn alac_sample_to_i16(s: i32) -> i16 {
    (s >> 16) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_left_aligned_samples() {
        // alac crate left-aligns every bit depth to 32 bits, so a 16-bit 0x1234 arrives as 0x1234_0000
        assert_eq!(alac_sample_to_i16(0x1234 << 16), 0x1234);
        assert_eq!(alac_sample_to_i16(-0x1234 << 16), -0x1234);
        assert_eq!(alac_sample_to_i16(0x1234 << 16), 0x1234);
        assert_eq!(alac_sample_to_i16(-0x1234 << 16), -0x1234);
    }
}

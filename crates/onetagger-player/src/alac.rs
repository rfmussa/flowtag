use anyhow::Error;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;
use rodio::Source;
use alac::{Reader, Samples, StreamInfo};

use crate::pcm::{CacheFill, DecodedPcm, PcmPlayback};

/// ALAC stream inside an MP4 container. Duration is reported by `MP4Source`.
pub struct ALACSource {
    path: PathBuf,
    stream_info: StreamInfo,
    /// Decoded audio, filled in the background on first `get_source` and reused for seeks / replays
    cache: CacheFill,
}

impl ALACSource {
    // Read stream info from file
    pub fn new(path: impl AsRef<Path>) -> Result<ALACSource, Error> {
        let file = File::open(&path)?;
        let reader = Reader::new(BufReader::new(file))?;
        Ok(ALACSource {
            path: path.as_ref().to_owned(),
            stream_info: reader.stream_info().to_owned(),
            cache: CacheFill::new(),
        })
    }

    /// Get rodio source: the cached PCM when the background decode has landed, otherwise
    /// a streaming decoder (instant start) while the cache fills in the background
    pub fn get_source(&self) -> Result<Box<dyn Source<Item = i16> + Send>, Error> {
        if let Some(pcm) = self.cache.get() {
            return Ok(Box::new(PcmPlayback::new(pcm)));
        }
        let stream = ALACStream::open(&self.path)?;
        let (path, stream_info) = (self.path.clone(), self.stream_info.clone());
        self.cache.spawn_fill(move || decode(&path, &stream_info));
        Ok(Box::new(stream))
    }
}

/// Decode the whole stream into interleaved i16 PCM, with its own reader
fn decode(path: &Path, stream_info: &StreamInfo) -> Result<DecodedPcm, Error> {
    let reader = Reader::new(BufReader::new(File::open(path)?))?;
    let samples = collect_samples(reader.into_samples::<i32>())
        .map_err(|e| anyhow!("{e} ({path:?})"))?;
    Ok(DecodedPcm {
        channels: stream_info.channels() as u16,
        sample_rate: stream_info.sample_rate(),
        samples,
    })
}

/// Convert decoded samples to i16; any packet error fails the decode
fn collect_samples<E: Debug>(stream: impl IntoIterator<Item = Result<i32, E>>) -> Result<Vec<i16>, Error> {
    stream.into_iter()
        .map(|s| s.map(alac_sample_to_i16).map_err(|e| anyhow!("Failed decoding ALAC: {e:?}")))
        .collect()
}

/// Streaming ALAC decoder, used until the PCM cache is filled. Not seekable in place:
/// the player's seek fallback re-creates the source, which is the cached PCM once filled.
/// Stops at the first bad packet.
struct ALACStream {
    samples: Samples<BufReader<File>, i32>,
    stream_info: StreamInfo,
}

impl ALACStream {
    fn open(path: &Path) -> Result<ALACStream, Error> {
        let reader = Reader::new(BufReader::new(File::open(path)?))?;
        let stream_info = reader.stream_info().to_owned();
        Ok(ALACStream { samples: reader.into_samples(), stream_info })
    }
}

impl Source for ALACStream {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { self.stream_info.channels() as u16 }
    fn sample_rate(&self) -> u32 { self.stream_info.sample_rate() }
    fn total_duration(&self) -> Option<Duration> { None }
}

impl Iterator for ALACStream {
    type Item = i16;
    fn next(&mut self) -> Option<i16> {
        match self.samples.next()? {
            Ok(s) => Some(alac_sample_to_i16(s)),
            Err(_) => None,
        }
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

use std::path::{PathBuf, Path};
use anyhow::Error;
use std::io::BufReader;
use std::fs::File;
use rodio::{Source, Decoder};
use crate::AudioSource;

pub struct WAVSource {
    path: PathBuf,
    duration: u128
}

impl WAVSource {
    pub fn new(path: impl AsRef<Path>) -> Result<WAVSource, Error> {
        Self::new_with_duration(path, None)
    }

    /// `duration_ms` (e.g. from the library DB) skips opening the decoder just to measure
    pub fn new_with_duration(path: impl AsRef<Path>, duration_ms: Option<u128>) -> Result<WAVSource, Error> {
        let mut wav = WAVSource {
            path: path.as_ref().to_owned(),
            duration: duration_ms.unwrap_or(0)
        };
        // Get duration from decoder
        if duration_ms.is_none() {
            wav.duration = wav.get_source()?.total_duration().ok_or(anyhow!("Missing duration"))?.as_millis();
        }

        Ok(wav)
    }
}

impl AudioSource for WAVSource {
    // Get duration
    fn duration(&self) -> u128 {
        self.duration
    }

    // Get rodio decoder
    fn get_source(&self) -> Result<Box<dyn Source<Item = i16> + Send>, Error> {
        Ok(Box::new(Decoder::new_wav(BufReader::new(File::open(&self.path)?))?))
    }
}
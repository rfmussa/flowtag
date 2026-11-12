use std::path::{Path, PathBuf};
use anyhow::Error;
use lofty::file::AudioFile;
use std::io::BufReader;
use std::fs::File;
use rodio::{Source, Decoder};
use crate::AudioSource;

pub struct MP3Source {
    path: PathBuf,
    duration: u128
}
impl MP3Source {
    pub fn new(path: impl AsRef<Path>) -> Result<MP3Source, Error> {
        Self::new_with_duration(path, None)
    }

    /// `duration_ms` (e.g. from the library DB) skips reading the file's metadata
    pub fn new_with_duration(path: impl AsRef<Path>, duration_ms: Option<u128>) -> Result<MP3Source, Error> {
        // Get duration
        let duration = match duration_ms {
            Some(ms) => ms,
            None => lofty::read_from_path(&path)?.properties().duration().as_millis(),
        };

        Ok(MP3Source {
            path: path.as_ref().to_owned(),
            duration
        })
    }
}

impl AudioSource for MP3Source {
    // Get duration
    fn duration(&self) -> u128 {
        self.duration
    }

    // Get rodio decoder
    fn get_source(&self) -> Result<Box<dyn Source<Item = i16> + Send>, Error> {
        Ok(Box::new(Decoder::new_mp3(BufReader::new(File::open(&self.path)?))?))
    }
}
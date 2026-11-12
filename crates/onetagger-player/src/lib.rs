#[macro_use] extern crate log;
#[macro_use] extern crate anyhow;

use anyhow::Error;
use std::io::Cursor;
use std::path::Path;
use std::sync::mpsc::{Sender, Receiver, channel};
use std::thread;
use std::time::Instant;
use hound::{WavSpec, SampleFormat, WavWriter};
use rodio::{Sink, Source};

/// Audio duration read with tag parsing disabled: a damaged text frame in the
/// metadata (e.g. an odd-length UTF-16 string) must not make a file unplayable
pub(crate) fn duration_without_tags(path: impl AsRef<Path>) -> Result<std::time::Duration, Error> {
    use lofty::file::AudioFile;
    let file = lofty::probe::Probe::open(&path)?
        .options(lofty::config::ParseOptions::new().read_tags(false))
        .read()?;
    Ok(file.properties().duration())
}

pub mod mp3;
pub mod mp4;
pub mod wav;
pub mod ogg;
pub mod alac;
pub mod flac;
pub mod aiff;
pub(crate) mod pcm;

/// Re-Export to prevent dependency issues
pub use rodio;

pub struct AudioPlayer {
    tx: Sender<PlayerAction>,
    rx: Receiver<bool>,
}

impl AudioPlayer {
    /// Create new instance
    pub fn new() -> AudioPlayer {
        // Create thread, becuase for some reason it stops working after closure ends
        let (tx_main, rx) = channel();
        let (tx, rx_main) = channel();
        thread::spawn(move || {
            let mut volume = 0.5;
            let mut source = None;
            // Create sink
            // Kept alive for the thread's lifetime; replaced on a Load when the default
            // output device changed (rodio never follows device changes on its own)
            let mut bound_device = default_output_name();
            let (mut _stream, mut stream_handle) = rodio::OutputStream::try_default().unwrap();
            let mut sink = Sink::try_new(&stream_handle).unwrap();
            sink.set_volume(volume);
            sink.pause();
            // Wait for messages
            for action in rx {
                match action {
                    PlayerAction::Volume(v) => {
                        sink.set_volume(v);
                        volume = v;
                    },
                    PlayerAction::Stop => {
                        sink.stop();
                    }
                    PlayerAction::Play => sink.play(),
                    PlayerAction::Pause => sink.pause(),
                    // Play new source
                    PlayerAction::Load(audio_source) => {
                        sink.stop();
                        // Rebind only when the default output changed (opening one costs ~220ms);
                        // on any failure keep the current device
                        match default_output_name() {
                            None => debug!("player output: default device name unreadable, keeping current device"),
                            Some(current) if should_rebind(bound_device.as_deref(), &current) => {
                                match rodio::OutputStream::try_default() {
                                    Ok((new_stream, new_handle)) => {
                                        _stream = new_stream;
                                        stream_handle = new_handle;
                                        bound_device = Some(current);
                                        info!("player output: rebound to default device");
                                    },
                                    Err(e) => warn!("player output: rebind failed, keeping current device: {e}"),
                                }
                            },
                            Some(_) => {},
                        }
                        // Create new sink
                        sink = Sink::try_new(&stream_handle).unwrap();
                        sink.set_volume(volume);
                        sink.pause();
                        // Append source
                        let decode_start = Instant::now();
                        if let Ok(s) = audio_source.get_source() {
                            sink.append(s);
                        }
                        info!("player decode: took {}ms", decode_start.elapsed().as_millis());
                        // Save source
                        source = Some(audio_source);
                    },
                    PlayerAction::Seek(pos) => {
                        let seek_start = Instant::now();
                        if source.is_none() {
                            // Always reply, or AudioPlayer::seek would block forever
                            tx.send(false).ok();
                            continue;
                        }
                        // In-place seek: all sources support it (symphonia decoders and
                        // the shared PCM buffer used by AIFF/ALAC), making seeks near-instant
                        if sink.try_seek(std::time::Duration::from_millis(pos)).is_ok() {
                            info!("player seek: {pos}ms in_place=true took {}ms", seek_start.elapsed().as_millis());
                            tx.send(!sink.is_paused()).ok();
                            continue;
                        }
                        // Safety fallback for any source without seek support:
                        // re-create the source and skip samples from the start
                        let paused = sink.is_paused();
                        sink.stop();
                        sink = Sink::try_new(&stream_handle).unwrap();
                        sink.set_volume(volume);
                        if paused {
                            sink.pause();
                        }
                        let s = source.as_ref().unwrap();
                        if let Ok(mut s) = s.get_source() {
                            let n_skip = s.sample_rate() as f32 * s.channels() as f32 * pos as f32 / 1000.0;
                            for _ in 0..n_skip as u64 {
                                if s.next().is_none() {
                                    break;
                                }
                            }
                            sink.append(s);
                        }
                        info!("player seek: {pos}ms in_place=false took {}ms", seek_start.elapsed().as_millis());
                        // Sync
                        tx.send(!sink.is_paused()).ok();
                    }
                }
            }
        });

        AudioPlayer {
            tx: tx_main,
            rx: rx_main
        }
    }

    // Load file
    pub fn load_file(&self, source: Box<dyn AudioSource + Send + 'static>) {
        self.tx.send(PlayerAction::Load(source)).ok();
    }

    pub fn play(&self) {
        self.tx.send(PlayerAction::Play).ok();
    }

    pub fn pause(&self) {
        self.tx.send(PlayerAction::Pause).ok();
    }

    pub fn seek(&self, pos: u64) -> bool {
        self.tx.send(PlayerAction::Seek(pos)).ok();
        // Wait for ready
        self.rx.recv().unwrap()
    }

    pub fn volume(&self, volume: f32) {
        self.tx.send(PlayerAction::Volume(volume)).ok();
    }

    pub fn stop(&self) {
        self.tx.send(PlayerAction::Stop).ok();
    }
}

/// Name of the system's current default output device (~0.5ms when warm)
fn default_output_name() -> Option<String> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};
    rodio::cpal::default_host().default_output_device().and_then(|d| d.name().ok())
}

/// Whether the output must be reopened: the default device differs from the bound one,
/// or the bound one's name was never known. Same-named devices are not told apart.
fn should_rebind(bound: Option<&str>, current: &str) -> bool {
    bound != Some(current)
}

enum PlayerAction {
    Play,
    Pause,
    Load(Box<dyn AudioSource + Send + 'static>),
    /// ms
    Seek(u64),
    /// 0.0 - 1.0
    Volume(f32),
    Stop,
}

/// Wrapper for getting audio sources
pub struct AudioSources {}
impl AudioSources {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Box<dyn AudioSource + Send + 'static>, Error> {
        Self::from_path_with_duration(path, None)
    }

    /// Like `from_path`, but a known duration (ms, e.g. from the library DB) skips the per-source duration probe
    pub fn from_path_with_duration(path: impl AsRef<Path>, duration_ms: Option<u128>) -> Result<Box<dyn AudioSource + Send + 'static>, Error> {
        let p = path.as_ref().extension().ok_or(anyhow!("Missing extension"))?.to_ascii_lowercase();
        // MP3
        if p == "mp3" {
            return Ok(Box::new(mp3::MP3Source::new_with_duration(path, duration_ms)?));
        }
        // FLAC
        if p == "flac" {
            return Ok(Box::new(flac::FLACSource::new_with_duration(path, duration_ms)?));
        }
        // AIFF
        if p == "aif" || p == "aiff" {
            return Ok(Box::new(aiff::AIFFSource::new_with_duration(path, duration_ms)?));
        }
        // MP4
        if p == "m4a" || p == "mp4" {
            return Ok(Box::new(mp4::MP4Source::new_with_duration(path, duration_ms)?));
        }
        // WAV
        if p == "wav" {
            return Ok(Box::new(wav::WAVSource::new_with_duration(path, duration_ms)?));
        }
        // OGG
        if p == "ogg" || p == "opus" || p == "oga" || p == "spx" {
            return Ok(Box::new(ogg::OGGSource::new(path)?));
        }

        Err(anyhow!("Unsupported format!").into())
    }
}

pub trait AudioSource {
    /// Duration in ms
    fn duration(&self) -> u128;
    /// Rodio Source
    fn get_source(&self) -> Result<Box<dyn Source<Item = i16> + Send>, Error>;

    /// Stream generate 2D waveform, in thread, stream
    fn generate_waveform(&self, bars: i16) -> Result<(Receiver<f32>, Sender<bool>), Error> {
        let source = self.get_source()?;
        // Calculate n samples per bar
        let sample_rate = source.sample_rate() as f32;
        let channels = source.channels() as f32;
        let duration = self.duration() as f32 / 1000.0;
        let n_samples = (sample_rate * channels * (duration / bars as f32)).round() as usize;

        // Create thread
        let (tx, rx) = channel();
        let (tx1, rx1) = channel();
        thread::spawn(move || {
            // Get samples
            let mut samples: Vec<i16> = vec![];
            for sample in source {
                // Cancel
                if rx1.try_recv().is_ok() {
                    break;
                }

                samples.push(sample);

                // Buffer full
                if samples.len() >= n_samples {
                    let sum: i64 = samples.iter().fold(0, |s, v| s + *v as i64);
                    let wave: f64 = sum as f64 / samples.len() as f64;
                    tx.send(((wave.abs() + 1.0).log2() / 10.0) as f32).ok();
                    samples = vec![];
                }
            }
        });

        // tx1 = for canceling
        Ok((rx, tx1))
    }

    /// Generate wav for streaming in browser
    fn generate_wav(&self) -> Result<Vec<u8>, Error> {
        let source = self.get_source()?;
        let spec = WavSpec {
            channels: source.channels(),
            sample_rate: source.sample_rate(),
            bits_per_sample: 16,
            sample_format: SampleFormat::Int
        };
        // Generate wav
        let mut buf = vec![];
        {
            let mut cursor = Cursor::new(&mut buf);
            let mut writer = WavWriter::new(&mut cursor, spec)?;
            for s in source {
                writer.write_sample(s)?;
            }
            writer.finalize()?;
        }
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::should_rebind;

    #[test]
    fn rebinds_only_when_default_device_changed() {
        assert!(!should_rebind(Some("MacBook Pro Speakers"), "MacBook Pro Speakers"));
        assert!(should_rebind(Some("MacBook Pro Speakers"), "Living Room AirPlay"));
        assert!(should_rebind(None, "MacBook Pro Speakers"), "unknown bound device rebinds");
    }
}

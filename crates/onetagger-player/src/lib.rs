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

#[cfg(target_os = "macos")]
mod output_watch;

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

/// Events the player thread pushes without being asked
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerEvent {
    /// The sink actually ran out of audio while playing — the exact end of
    /// the track, independent of any stated duration
    Ended,
}

/// A playing sink that ran dry is the end of the track; it is reported once
/// per load (a paused sink, or one that already reported, stays quiet)
fn should_emit_ended(has_source: bool, paused: bool, empty: bool, already_sent: bool) -> bool {
    has_source && !paused && empty && !already_sent
}

pub struct AudioPlayer {
    tx: Sender<PlayerAction>,
    rx: Receiver<bool>,
    events: Option<Receiver<PlayerEvent>>,
}

impl AudioPlayer {
    /// Create new instance
    pub fn new() -> AudioPlayer {
        // Create thread, becuase for some reason it stops working after closure ends
        let (tx_main, rx) = channel();
        let (tx, rx_main) = channel();
        let (event_tx, event_rx) = channel();
        // The player posts rebinds to itself (sleep recovery, device watcher);
        // the flag collapses a burst of device-change events into one rebind
        let self_tx = tx_main.clone();
        let rebind_pending = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        #[cfg(target_os = "macos")]
        {
            let watcher_tx = tx_main.clone();
            let pending = rebind_pending.clone();
            let registered = output_watch::watch_default_output(move || {
                if !pending.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    watcher_tx.send(PlayerAction::RebindOutput).ok();
                }
            });
            if registered {
                info!("player output: watching default-device changes");
            } else {
                warn!("player output: device watcher registration failed, falling back to per-load checks");
            }
        }
        thread::spawn(move || {
            let mut volume = 0.5;
            let mut source = None;
            // Create sink
            // Kept alive for the thread's lifetime; replaced on a Load when the default
            // output device changed (rodio never follows device changes on its own)
            let mut bound_device = default_output_name();
            // Sleep detector: the monotonic clock stops during system sleep,
            // the wall clock does not — skew between actions means a sleep,
            // and sleep kills the CoreAudio stream without renaming the device
            let mut last_action_mono = Instant::now();
            let mut last_action_wall = std::time::SystemTime::now();
            let (mut _stream, mut stream_handle) = rodio::OutputStream::try_default().unwrap();
            let mut sink = Sink::try_new(&stream_handle).unwrap();
            sink.set_volume(volume);
            sink.pause();
            // One Ended per load; starts true because nothing is loaded yet
            let mut ended_sent = true;
            // Wait for messages; the timeout tick watches for the sink running dry
            loop {
                let action = match rx.recv_timeout(std::time::Duration::from_millis(200)) {
                    Ok(action) => action,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if should_emit_ended(source.is_some(), sink.is_paused(), sink.empty(), ended_sent) {
                            ended_sent = true;
                            info!("player ended: sink ran dry");
                            event_tx.send(PlayerEvent::Ended).ok();
                        }
                        continue;
                    },
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                };
                let slept = slept_between(last_action_mono.elapsed(), last_action_wall.elapsed().unwrap_or_default());
                last_action_mono = Instant::now();
                last_action_wall = std::time::SystemTime::now();
                match action {
                    PlayerAction::Volume(v) => {
                        sink.set_volume(v);
                        volume = v;
                    },
                    PlayerAction::Stop => {
                        sink.stop();
                        // An explicitly stopped track is not an end-of-track
                        ended_sent = true;
                    }
                    PlayerAction::Play => {
                        // Resuming after a system sleep would play into the dead
                        // stream: rebind first (both messages land before any other
                        // producer, and the re-sent Play sees no further skew)
                        if slept {
                            self_tx.send(PlayerAction::RebindOutput).ok();
                            self_tx.send(PlayerAction::Play).ok();
                        } else {
                            sink.play();
                        }
                    },
                    PlayerAction::Pause => sink.pause(),
                    // Play new source
                    PlayerAction::Load(audio_source) => {
                        // One sink for the deck's lifetime: clear and reuse it
                        // rather than replacing it. A replaced sink can outlive
                        // its stop() by a beat and keep sounding under the new
                        // track (heard as "plays the previous track until a
                        // seek"); with a single sink there is no predecessor.
                        sink.clear();
                        // Rebind when the default output changed (opening one costs ~220ms)
                        // or the machine slept since the last action; on any failure keep
                        // the current device
                        let current = default_output_name();
                        let rebind = match &current {
                            None => {
                                debug!("player output: default device name unreadable{}", if slept { ", rebinding after sleep anyway" } else { ", keeping current device" });
                                slept
                            },
                            Some(c) => slept || should_rebind(bound_device.as_deref(), c),
                        };
                        if rebind {
                            match rodio::OutputStream::try_default() {
                                Ok((new_stream, new_handle)) => {
                                    _stream = new_stream;
                                    stream_handle = new_handle;
                                    bound_device = current;
                                    // A new stream needs a new sink; dropping the
                                    // old stream silences it at the device level
                                    sink = Sink::try_new(&stream_handle).unwrap();
                                    sink.set_volume(volume);
                                    sink.pause();
                                    info!("player output: rebound to default device{}", if slept { " (system slept since last load)" } else { "" });
                                },
                                Err(e) => warn!("player output: rebind failed, keeping current device: {e}"),
                            }
                        }
                        // Append source (clear() left the sink paused)
                        let decode_start = Instant::now();
                        if let Ok(s) = audio_source.get_source() {
                            sink.append(s);
                        }
                        info!("player decode: took {}ms", decode_start.elapsed().as_millis());
                        // Save source
                        source = Some(audio_source);
                        // Arm end detection for this load (an unplayable file
                        // counts as instantly ended, so autoplay skips past it)
                        ended_sent = false;
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
                        // Same one-sink rule as Load: clear and reuse
                        sink.clear();
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
                        if !paused {
                            sink.play();
                        }
                        info!("player seek: {pos}ms in_place=false took {}ms", seek_start.elapsed().as_millis());
                        // Sync
                        tx.send(!sink.is_paused()).ok();
                    }
                    PlayerAction::RebindOutput => {
                        rebind_pending.store(false, std::sync::atomic::Ordering::Relaxed);
                        let pos = sink.get_pos();
                        let paused = sink.is_paused();
                        match rodio::OutputStream::try_default() {
                            Ok((new_stream, new_handle)) => {
                                _stream = new_stream;
                                stream_handle = new_handle;
                                bound_device = default_output_name();
                                sink.stop();
                                sink = Sink::try_new(&stream_handle).unwrap();
                                sink.set_volume(volume);
                                sink.pause();
                                if let Some(src) = source.as_ref() {
                                    if let Ok(s) = src.get_source() {
                                        sink.append(s);
                                        // Position restore; a source without try_seek restarts at 0
                                        sink.try_seek(pos).ok();
                                    }
                                }
                                if !paused {
                                    sink.play();
                                }
                                info!("player output: moved playback to the default device at {}ms{}", pos.as_millis(), if paused { " (paused)" } else { "" });
                            },
                            Err(e) => warn!("player output: rebind failed, keeping current device: {e}"),
                        }
                    }
                }
            }
        });

        AudioPlayer {
            tx: tx_main,
            rx: rx_main,
            events: Some(event_rx),
        }
    }

    /// The push-event stream (Ended, …); yields once, to whoever forwards the
    /// events — the channel closes when the player is dropped
    pub fn take_events(&mut self) -> Option<Receiver<PlayerEvent>> {
        self.events.take()
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

/// System sleep stops the monotonic clock but not the wall clock, so a wall
/// gap well beyond the monotonic gap means the machine slept in between —
/// and sleep tears down CoreAudio streams without changing the device name
const SLEEP_SKEW_THRESHOLD: std::time::Duration = std::time::Duration::from_secs(30);
fn slept_between(monotonic_gap: std::time::Duration, wall_gap: std::time::Duration) -> bool {
    wall_gap.saturating_sub(monotonic_gap) > SLEEP_SKEW_THRESHOLD
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
    /// The default output device changed (or the system slept): move playback
    /// to the current default, keeping position and pause state
    RebindOutput,
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
    use super::{should_emit_ended, should_rebind, slept_between};
    use std::time::Duration;

    #[test]
    fn rebinds_only_when_default_device_changed() {
        assert!(!should_rebind(Some("MacBook Pro Speakers"), "MacBook Pro Speakers"));
        assert!(should_rebind(Some("MacBook Pro Speakers"), "Living Room AirPlay"));
        assert!(should_rebind(None, "MacBook Pro Speakers"), "unknown bound device rebinds");
    }

    #[test]
    fn ended_fires_once_for_a_playing_sink_that_ran_dry() {
        // (has_source, paused, empty, already_sent)
        assert!(should_emit_ended(true, false, true, false));
        assert!(!should_emit_ended(true, false, true, true), "only once per load");
        assert!(!should_emit_ended(true, true, true, false), "paused is not ended");
        assert!(!should_emit_ended(true, false, false, false), "still playing");
        assert!(!should_emit_ended(false, false, true, false), "nothing loaded");
    }

    #[test]
    #[ignore] // needs an audio output device: cargo test -- --ignored
    fn ended_event_reaches_the_receiver_when_audio_runs_out() {
        struct Blip;
        impl crate::AudioSource for Blip {
            fn duration(&self) -> u128 { 100 }
            fn get_source(&self) -> Result<Box<dyn rodio::Source<Item = i16> + Send>, anyhow::Error> {
                // 100 ms of silence at 8 kHz mono
                Ok(Box::new(rodio::buffer::SamplesBuffer::new(1, 8000, vec![0i16; 800])))
            }
        }
        let mut player = crate::AudioPlayer::new();
        let events = player.take_events().expect("events receiver");
        player.load_file(Box::new(Blip));
        player.play();
        let event = events.recv_timeout(std::time::Duration::from_secs(3));
        assert!(matches!(event, Ok(crate::PlayerEvent::Ended)), "got {event:?}");
    }

    #[test]
    fn sleep_is_a_wall_clock_gap_the_monotonic_clock_missed() {
        // Overnight: monotonic saw seconds, wall clock saw hours
        assert!(slept_between(Duration::from_secs(5), Duration::from_secs(17 * 3600)));
        // Normal session: the clocks agree (small scheduling skew is not sleep)
        assert!(!slept_between(Duration::from_secs(600), Duration::from_secs(605)));
        // Wall clock stepped backwards (NTP): not a sleep
        assert!(!slept_between(Duration::from_secs(600), Duration::from_secs(500)));
    }
}

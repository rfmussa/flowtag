use anyhow::Error;
use onetagger_player::{AudioPlayer, AudioSource};
use onetagger_player::rodio::Source;
use onetagger_player::rodio::buffer::SamplesBuffer;

/// Seeking with nothing loaded must still reply (false), never block forever.
/// Needs an audio output device because `AudioPlayer::new` opens one.
#[test]
fn seek_without_source_replies_false() {
    let player = AudioPlayer::new();
    assert!(!player.seek(1000));
}

struct Tone;
impl AudioSource for Tone {
    fn duration(&self) -> u128 { 100 }
    fn get_source(&self) -> Result<Box<dyn Source<Item = i16> + Send>, Error> {
        Ok(Box::new(SamplesBuffer::new(2, 1000, vec![0i16; 200])))
    }
}

/// Each load rebuilds the output stream; two loads in a row must not panic, and the
/// player thread must still answer afterwards. Needs an audio output device.
#[test]
fn loads_twice_and_still_replies() {
    let player = AudioPlayer::new();
    player.load_file(Box::new(Tone));
    player.load_file(Box::new(Tone));
    // Paused after load, so the in-place seek replies "not playing"
    assert!(!player.seek(50));
}

use onetagger_player::AudioPlayer;

/// Seeking with nothing loaded must still reply (false), never block forever.
/// Needs an audio output device because `AudioPlayer::new` opens one.
#[test]
fn seek_without_source_replies_false() {
    let player = AudioPlayer::new();
    assert!(!player.seek(1000));
}

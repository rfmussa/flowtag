//! Track inspector metadata roundtrip against real audio files:
//! write fields through meta::write_track_meta, read them back from the file,
//! and confirm the DB row follows.

use std::path::Path;
use std::process::Command;
use onetagger_db::{meta, scan, tracks, Db};

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn gen_audio(dir: &Path, name: &str, extra: &[&str]) -> bool {
    let out = dir.join(name);
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-f", "lavfi", "-i", "sine=frequency=440:duration=0.5", "-y"]);
    cmd.args(extra);
    cmd.arg(&out);
    cmd.output().map(|o| o.status.success() && out.exists()).unwrap_or(false)
}

#[test]
fn meta_roundtrip_mp3_and_flac() {
    if !ffmpeg_available() {
        eprintln!("SKIP: ffmpeg not available");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    std::fs::create_dir(&music).unwrap();
    assert!(gen_audio(&music, "track.mp3", &["-codec:a", "libmp3lame", "-b:a", "64k"]));
    assert!(gen_audio(&music, "track.flac", &[]));

    let db = Db::open(dir.path().join("test.db")).unwrap();
    tracks::add_folder(&db, &music.to_string_lossy()).unwrap();
    scan::scan(&db, false, |_, _| {}).unwrap();

    for track in tracks::list_tracks(&db).unwrap() {
        let new = meta::TrackMeta {
            title: "Put Your Back In It".into(),
            artist: "Peven Everett".into(),
            album: "Studio Confessions".into(),
            genre: "Soulful House".into(),
            year: Some(2002),
            bpm: Some(126),
            key: "7A".into(),
            comment: "crowd melter".into(),
        };
        let echoed = meta::write_track_meta(&db, track.id, &new).unwrap();
        assert_eq!(echoed.title, new.title, "{}", track.path);

        // Fresh read from the file itself
        let read = meta::read_track_meta(&track.path).unwrap();
        assert_eq!(read.title, new.title, "{}", track.path);
        assert_eq!(read.artist, new.artist, "{}", track.path);
        assert_eq!(read.album, new.album, "{}", track.path);
        assert_eq!(read.genre, new.genre, "{}", track.path);
        assert_eq!(read.year, new.year, "{}", track.path);
        assert_eq!(read.bpm, new.bpm, "{}", track.path);
        assert_eq!(read.key, new.key, "{}", track.path);
        assert_eq!(read.comment, new.comment, "{}", track.path);

        // DB row follows (title/artist/bpm/key/year live in the list view)
        let row = tracks::list_tracks(&db).unwrap().into_iter().find(|t| t.id == track.id).unwrap();
        assert_eq!(row.title, new.title);
        assert_eq!(row.artist, new.artist);
        assert_eq!(row.bpm, Some(126));
        assert_eq!(row.key.as_deref(), Some("7A"));
        assert_eq!(row.year, Some(2002));
        // Self-write must not look like an external change to the scanner
        let res = scan::scan(&db, false, |_, _| {}).unwrap();
        assert_eq!(res.updated, 0, "self-write of {} retriggered a scan parse", track.path);

        // Clearing a field removes it from the file (year included)
        let cleared = meta::TrackMeta { comment: String::new(), year: None, ..new.clone() };
        meta::write_track_meta(&db, track.id, &cleared).unwrap();
        let read = meta::read_track_meta(&track.path).unwrap();
        assert_eq!(read.comment, "", "{}", track.path);
        assert_eq!(read.year, None, "year not cleared in {}", track.path);
        assert_eq!(read.artist, cleared.artist, "{}", track.path);
    }
}

#[test]
fn empty_title_falls_back_to_filename_stem() {
    if !ffmpeg_available() {
        eprintln!("SKIP: ffmpeg not available");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    std::fs::create_dir(&music).unwrap();
    assert!(gen_audio(&music, "stemname.mp3", &["-codec:a", "libmp3lame", "-b:a", "64k"]));

    let db = Db::open(dir.path().join("test.db")).unwrap();
    tracks::add_folder(&db, &music.to_string_lossy()).unwrap();
    scan::scan(&db, false, |_, _| {}).unwrap();
    let track = tracks::list_tracks(&db).unwrap().pop().unwrap();

    let meta_in = meta::TrackMeta { title: "  ".into(), ..Default::default() };
    meta::write_track_meta(&db, track.id, &meta_in).unwrap();
    let row = tracks::list_tracks(&db).unwrap().pop().unwrap();
    assert_eq!(row.title, "stemname", "DB title must never be empty");
}

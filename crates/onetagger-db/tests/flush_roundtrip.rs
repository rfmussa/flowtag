//! End-to-end flusher test against real audio files generated with ffmpeg:
//! scan -> tag in DB -> flush to files -> rescan shows no self-changes ->
//! fresh DB re-imports the same tags from the files.

use std::path::Path;
use std::process::Command;
use onetagger_db::{flush, scan, tags, tracks, Db, FILE_TAG_FIELD};

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Generate a 0.5s silent audio file; returns false when the encoder is unavailable
fn gen_audio(dir: &Path, name: &str, extra: &[&str]) -> bool {
    let out = dir.join(name);
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-f", "lavfi", "-i", "sine=frequency=440:duration=0.5", "-y"]);
    cmd.args(extra);
    cmd.arg(&out);
    cmd.output().map(|o| o.status.success() && out.exists()).unwrap_or(false)
}

#[test]
fn flush_roundtrip_all_formats() {
    if !ffmpeg_available() {
        eprintln!("SKIP: ffmpeg not available");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    std::fs::create_dir(&music).unwrap();

    // Generate one file per supported format
    let mut names: Vec<&str> = vec![];
    for (name, extra) in [
        ("track.mp3", vec!["-codec:a", "libmp3lame", "-b:a", "64k"]),
        ("track.flac", vec![]),
        ("track.m4a", vec!["-codec:a", "aac"]),
        ("track.wav", vec![]),
        ("track.aiff", vec![]),
    ] {
        let extra: Vec<&str> = extra;
        if gen_audio(&music, name, &extra) {
            names.push(name);
        } else {
            eprintln!("SKIP format (ffmpeg could not encode): {name}");
        }
    }
    assert!(names.len() >= 4, "expected ffmpeg to encode most formats, got {names:?}");

    // Scan into the DB
    let db = Db::open(dir.path().join("test.db")).unwrap();
    tracks::add_folder(&db, &music.to_string_lossy()).unwrap();
    let res = scan::scan(&db, false, |_, _| {}).unwrap();
    assert_eq!(res.added, names.len());

    // Tag every track in the DB
    let all = tracks::list_tracks(&db).unwrap();
    for t in &all {
        tags::add_tag_to_track(&db, t.id, "Deep House").unwrap();
        tags::add_tag_to_track(&db, t.id, "peak-time").unwrap();
    }

    // Flush immediately (no debounce): every track written, no errors
    let events = flush::flush_pass(&db, 0, &|_| false).unwrap();
    assert_eq!(events.len(), all.len(), "all tracks flushed: {events:?}");
    assert!(events.iter().all(|e| e.ok), "flush errors: {events:?}");
    let (pending, errors) = flush::flush_status(&db).unwrap();
    assert_eq!(pending, 0);
    assert!(errors.is_empty());

    // Files round-trip: the TAGS field reads back as the two names
    for t in &all {
        let tag_wrap = onetagger_tag::Tag::load_file(&t.path, false).unwrap();
        let raw = tag_wrap.tag().get_raw(FILE_TAG_FIELD)
            .unwrap_or_else(|| panic!("no TAGS field written in {}", t.path));
        let parsed = scan::split_file_tags(&raw);
        assert_eq!(parsed, vec!["Deep House", "peak-time"], "bad TAGS in {}", t.path);
    }

    // Rescan: our own writes must NOT look like external changes
    let res = scan::scan(&db, false, |_, _| {}).unwrap();
    assert_eq!((res.added, res.updated, res.removed), (0, 0, 0), "self-triggered rescan changes");

    // Files still decodable after the write (duration survives)
    for t in &all {
        let parsed = lofty::read_from_path(&t.path);
        assert!(parsed.is_ok(), "file unreadable after tag write: {}", t.path);
    }

    // A fresh DB imports the same tags back from the files
    let db2 = Db::open(dir.path().join("fresh.db")).unwrap();
    tracks::add_folder(&db2, &music.to_string_lossy()).unwrap();
    scan::scan(&db2, false, |_, _| {}).unwrap();
    let pool: Vec<String> = tags::list_tags(&db2).unwrap().into_iter().map(|t| t.name).collect();
    assert_eq!(pool, vec!["Deep House", "peak-time"]);

    // Removing all tags flushes an empty field away
    let t0 = &tracks::list_tracks(&db).unwrap()[0];
    let pool = tags::list_tags(&db).unwrap();
    for tag in &pool {
        tags::remove_tag_from_track(&db, t0.id, tag.id).unwrap();
    }
    let events = flush::flush_pass(&db, 0, &|_| false).unwrap();
    assert!(events.iter().all(|e| e.ok));
    let tag_wrap = onetagger_tag::Tag::load_file(&t0.path, false).unwrap();
    assert!(tag_wrap.tag().get_raw(FILE_TAG_FIELD).is_none(), "TAGS field not removed from {}", t0.path);
}

#[test]
fn flush_pass_respects_skip() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(dir.path().join("test.db")).unwrap();
    {
        let conn = db.conn();
        conn.execute("INSERT INTO folders (id, path) VALUES (1, '/r')", []).unwrap();
        conn.execute(
            "INSERT INTO tracks (folder_id, path, format, title, mtime, size) VALUES (1, '/r/a.mp3', 'mp3', 'A', 0, 0)",
            [],
        ).unwrap();
    }
    let id = tracks::list_tracks(&db).unwrap()[0].id;
    tags::add_tag_to_track(&db, id, "x").unwrap();

    // Skipped file: untouched, not an error, still pending
    let events = flush::flush_pass(&db, 0, &|path| path == "/r/a.mp3").unwrap();
    assert!(events.is_empty());
    let (pending, errors) = flush::flush_status(&db).unwrap();
    assert_eq!(pending, 1);
    assert!(errors.is_empty());

    // Not skipped: the file doesn't exist, so this records an error + backoff
    let events = flush::flush_pass(&db, 0, &|_| false).unwrap();
    assert_eq!(events.len(), 1);
    assert!(!events[0].ok);
    let (_, errors) = flush::flush_status(&db).unwrap();
    assert_eq!(errors.len(), 1);
}

#[test]
fn expedite_makes_dirty_tracks_due() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(dir.path().join("test.db")).unwrap();
    {
        let conn = db.conn();
        conn.execute("INSERT INTO folders (id, path) VALUES (1, '/r')", []).unwrap();
        conn.execute(
            "INSERT INTO tracks (folder_id, path, format, title, mtime, size) VALUES (1, '/r/a.mp3', 'mp3', 'A', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO tracks (folder_id, path, format, title, mtime, size) VALUES (1, '/r/b.mp3', 'mp3', 'B', 0, 0)",
            [],
        ).unwrap();
    }
    let all = tracks::list_tracks(&db).unwrap();
    let (dirty, clean) = (all[0].id, all[1].id);
    tags::add_tag_to_track(&db, dirty, "x").unwrap();

    // Within the debounce window nothing is due
    assert!(flush::due_for_flush(&db, onetagger_db_now(), 5, 16).unwrap().is_empty());

    // Expedite the dirty track: due immediately. The clean track must NOT become dirty.
    flush::expedite(&db, Some(dirty)).unwrap();
    let due = flush::due_for_flush(&db, onetagger_db_now(), 5, 16).unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].track_id, dirty);
    flush::expedite(&db, Some(clean)).unwrap();
    let (pending, _) = flush::flush_status(&db).unwrap();
    assert_eq!(pending, 1, "expedite must never dirty a clean track");
}

/// Current unix seconds (the crate's internal now() is private)
fn onetagger_db_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

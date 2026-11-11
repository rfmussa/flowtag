#![allow(dead_code)]
use onetagger_db::Db;
use rusqlite::Connection;
use tempfile::TempDir;

/// Temp-dir backed DB; keep the `TempDir` alive for the test's duration.
pub struct TestDb {
    pub db: Db,
    _dir: TempDir,
}

impl TestDb {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("test.db")).unwrap();
        db.conn()
            .execute("INSERT INTO folders (path) VALUES ('/music')", [])
            .unwrap();
        TestDb { db, _dir: dir }
    }
}

pub fn add_track(c: &Connection, name: &str) -> i64 {
    let folder_id: i64 = c.query_row("SELECT id FROM folders LIMIT 1", [], |r| r.get(0)).unwrap();
    c.execute(
        "INSERT INTO tracks (folder_id, path, format, title, mtime, size) VALUES (?1, ?2, 'mp3', ?3, 0, 0)",
        rusqlite::params![folder_id, format!("/music/{name}.mp3"), name],
    )
    .unwrap();
    c.last_insert_rowid()
}

pub fn add_group(c: &Connection, name: &str) -> i64 {
    c.execute("INSERT INTO groups (name) VALUES (?1)", [name]).unwrap();
    c.last_insert_rowid()
}

pub fn add_tag(c: &Connection, name: &str, color: &str, group_id: Option<i64>) -> i64 {
    c.execute(
        "INSERT INTO tags (name, color, group_id) VALUES (?1, ?2, ?3)",
        rusqlite::params![name, color, group_id],
    )
    .unwrap();
    c.last_insert_rowid()
}

pub fn tag_track(c: &Connection, track: i64, tag: i64) {
    c.execute("INSERT INTO track_tags (track_id, tag_id) VALUES (?1, ?2)", [track, tag]).unwrap();
    c.execute("UPDATE tracks SET tag_version = tag_version + 1 WHERE id = ?1", [track]).unwrap();
}

pub fn untag_track(c: &Connection, track: i64, tag: i64) {
    c.execute("DELETE FROM track_tags WHERE track_id = ?1 AND tag_id = ?2", [track, tag]).unwrap();
    c.execute("UPDATE tracks SET tag_version = tag_version + 1 WHERE id = ?1", [track]).unwrap();
}

pub fn set_item(c: &Connection, track: i64, item: Option<&str>) {
    c.execute(
        "INSERT OR REPLACE INTO tangle_items (track_id, item_id, checked_at) VALUES (?1, ?2, 0)",
        rusqlite::params![track, item],
    )
    .unwrap();
}

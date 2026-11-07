use anyhow::Error;
use rusqlite::Connection;

use crate::tags::{DEFAULT_TAG_COLOR, TAG_COLOR_PALETTE};

/// A migration step: plain SQL, or code when the change needs Rust-side data
/// (e.g. the shared color palette).
enum Migration {
    Sql(&'static str),
    Code(fn(&Connection) -> Result<(), Error>),
}

/// v4: tags created before auto-coloring all wear the old teal default and are
/// indistinguishable. Re-spread them across the palette (custom colors stay).
/// v5: add tracks.added_at. Pre-existing rows take their file mtime as the
/// best available approximation of when they entered the library.
/// Idempotent: safe to re-run on a database that already has the column.
fn add_added_at(conn: &Connection) -> Result<(), Error> {
    let has_column = conn
        .prepare("SELECT 1 FROM pragma_table_info('tracks') WHERE name = 'added_at'")?
        .exists([])?;
    if !has_column {
        conn.execute_batch("ALTER TABLE tracks ADD COLUMN added_at INTEGER NOT NULL DEFAULT 0;")?;
    }
    conn.execute("UPDATE tracks SET added_at = mtime WHERE added_at = 0", [])?;
    Ok(())
}

fn recolor_legacy_teal_tags(conn: &Connection) -> Result<(), Error> {
    let mut stmt = conn.prepare("SELECT id FROM tags WHERE color = ?1 ORDER BY id")?;
    let ids: Vec<i64> = stmt
        .query_map([DEFAULT_TAG_COLOR], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for (i, id) in ids.iter().enumerate() {
        let color = TAG_COLOR_PALETTE[i % TAG_COLOR_PALETTE.len()];
        conn.execute("UPDATE tags SET color = ?2 WHERE id = ?1", rusqlite::params![id, color])?;
    }
    Ok(())
}

/// Ordered list of migrations; `user_version` records how many have been applied.
const MIGRATIONS: &[Migration] = &[
    Migration::Sql(
    // v1: initial schema
    "
    CREATE TABLE folders (
        id INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE
    );
    CREATE TABLE tracks (
        id INTEGER PRIMARY KEY,
        folder_id INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
        path TEXT NOT NULL UNIQUE,
        format TEXT NOT NULL,
        title TEXT NOT NULL,
        artist TEXT NOT NULL DEFAULT '',
        duration_ms INTEGER,
        mtime INTEGER NOT NULL,
        size INTEGER NOT NULL,
        missing INTEGER NOT NULL DEFAULT 0,
        needs_flush INTEGER NOT NULL DEFAULT 0,
        tag_updated_at INTEGER NOT NULL DEFAULT 0,
        flush_attempts INTEGER NOT NULL DEFAULT 0,
        next_flush_attempt_at INTEGER NOT NULL DEFAULT 0,
        last_flush_error TEXT
    );
    CREATE INDEX idx_tracks_flush ON tracks(needs_flush) WHERE needs_flush = 1;
    CREATE TABLE groups (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        color TEXT NOT NULL DEFAULT '#888888',
        position INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE tags (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE COLLATE NOCASE,
        color TEXT NOT NULL DEFAULT '#00d2bf',
        group_id INTEGER REFERENCES groups(id) ON DELETE SET NULL,
        keybind TEXT
    );
    CREATE TABLE track_tags (
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
        position INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (track_id, tag_id)
    );
    CREATE INDEX idx_track_tags_tag ON track_tags(tag_id);
    CREATE TABLE waveforms (
        track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
        bars BLOB NOT NULL
    );
    "),
    // v2: tag_version guards the flusher against clearing edits made while a
    // flush batch was in flight (bumped on every tag change of a track)
    Migration::Sql("ALTER TABLE tracks ADD COLUMN tag_version INTEGER NOT NULL DEFAULT 0;"),
    // v3: DJ metadata columns. Existing rows get stats reset so the next scan
    // re-parses every file once and backfills them.
    Migration::Sql("
    ALTER TABLE tracks ADD COLUMN bpm INTEGER;
    ALTER TABLE tracks ADD COLUMN key TEXT;
    ALTER TABLE tracks ADD COLUMN year INTEGER;
    UPDATE tracks SET mtime = 0, size = -1;
    "),
    // v4: spread legacy teal-default tags across the color palette
    Migration::Code(recolor_legacy_teal_tags),
    // v5: tracks remember when they entered the library
    Migration::Code(add_added_at),
];

/// Apply pending migrations. Idempotent: already-applied migrations are skipped
/// based on PRAGMA user_version.
pub fn migrate(conn: &Connection) -> Result<(), Error> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, migration) in MIGRATIONS.iter().enumerate() {
        let target = (i + 1) as i64;
        if version < target {
            match migration {
                Migration::Sql(sql) => conn.execute_batch(sql)?,
                Migration::Code(f) => f(conn)?,
            }
            conn.pragma_update(None, "user_version", target)?;
            info!("Applied DB migration v{target}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::tags::{DEFAULT_TAG_COLOR, TAG_COLOR_PALETTE};
    use crate::Db;

    /// v4 recolors tags still wearing the old teal default into distinct
    /// palette colors, and leaves custom colors alone.
    #[test]
    fn v4_recolors_legacy_teal_tags() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let db = Db::open(&path).unwrap();
        {
            let conn = db.conn();
            // Simulate a pre-v4 library: several teal tags + one custom
            for name in ["dark", "rolling", "peak", "warm"] {
                conn.execute(
                    "INSERT INTO tags (name, color) VALUES (?1, ?2)",
                    rusqlite::params![name, DEFAULT_TAG_COLOR],
                ).unwrap();
            }
            conn.execute("INSERT INTO tags (name, color) VALUES ('custom', '#123456')", []).unwrap();
            // Roll back to v3 and re-run migrations
            conn.pragma_update(None, "user_version", 3).unwrap();
            super::migrate(&conn).unwrap();

            let mut stmt = conn.prepare("SELECT name, color FROM tags ORDER BY id").unwrap();
            let rows: Vec<(String, String)> = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            let recolored: Vec<&(String, String)> = rows.iter().filter(|(n, _)| n != "custom").collect();
            for (name, color) in &recolored {
                assert_ne!(color, DEFAULT_TAG_COLOR, "{name} still teal after v4");
                assert!(TAG_COLOR_PALETTE.contains(&color.as_str()), "{name} got non-palette color {color}");
            }
            let unique: std::collections::HashSet<&String> = recolored.iter().map(|(_, c)| c).collect();
            assert_eq!(unique.len(), recolored.len(), "recolored tags must be distinct: {rows:?}");
            // Custom color untouched
            assert!(rows.iter().any(|(n, c)| n == "custom" && c == "#123456"));
        }
    }

    #[test]
    fn migrations_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        // Open twice: second open re-runs migrate() on the existing file
        drop(Db::open(&path).unwrap());
        let db = Db::open(&path).unwrap();
        // Schema usable
        let conn = db.conn();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    /// v5 adds added_at; rows that predate it inherit their file mtime as the
    /// best available "date added", and the migration is idempotent.
    #[test]
    fn v5_backfills_added_at_from_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("test.db")).unwrap();
        let conn = db.conn();
        conn.execute("INSERT INTO folders (path) VALUES ('/x')", []).unwrap();
        conn.execute(
            "INSERT INTO tracks (folder_id, path, format, title, mtime, size, added_at)
             VALUES (1, '/x/a.mp3', 'mp3', 'A', 1700000000, 10, 0)",
            [],
        ).unwrap();
        // Roll back and re-run: the backfill must fill the zeroed row
        conn.pragma_update(None, "user_version", 4).unwrap();
        super::migrate(&conn).unwrap();
        let added: i64 = conn
            .query_row("SELECT added_at FROM tracks WHERE path = '/x/a.mp3'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(added, 1_700_000_000, "pre-v5 rows take mtime as added_at");
    }

    /// Newly scanned tracks record the moment they entered the library
    #[test]
    fn new_tracks_get_added_at_now() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("test.db")).unwrap();
        let folder_id = {
            let conn = db.conn();
            conn.execute("INSERT INTO folders (path) VALUES ('/x')", []).unwrap();
            conn.last_insert_rowid()
        };
        let track = crate::ScannedTrack {
            path: "/x/new.mp3".into(),
            format: "mp3".into(),
            title: "New".into(),
            artist: "".into(),
            duration_ms: None,
            bpm: None,
            key: None,
            year: None,
            mtime: 1_600_000_000,
            size: 10,
            tag_names: vec![],
        };
        crate::tracks::upsert_tracks(&db, folder_id, &[track]).unwrap();
        let added: i64 = db.conn()
            .query_row("SELECT added_at FROM tracks WHERE path = '/x/new.mp3'", [], |r| r.get(0))
            .unwrap();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        assert!((added - now).abs() < 60, "added_at should be scan time, got {added}");
    }
}

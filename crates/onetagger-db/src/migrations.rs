use anyhow::Error;
use rusqlite::Connection;

/// Ordered list of migrations; `user_version` records how many have been applied.
const MIGRATIONS: &[&str] = &[
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
    ",
    // v2: tag_version guards the flusher against clearing edits made while a
    // flush batch was in flight (bumped on every tag change of a track)
    "ALTER TABLE tracks ADD COLUMN tag_version INTEGER NOT NULL DEFAULT 0;",
    // v3: DJ metadata columns. Existing rows get stats reset so the next scan
    // re-parses every file once and backfills them.
    "
    ALTER TABLE tracks ADD COLUMN bpm INTEGER;
    ALTER TABLE tracks ADD COLUMN key TEXT;
    ALTER TABLE tracks ADD COLUMN year INTEGER;
    UPDATE tracks SET mtime = 0, size = -1;
    ",
];

/// Apply pending migrations. Idempotent: already-applied migrations are skipped
/// based on PRAGMA user_version.
pub fn migrate(conn: &Connection) -> Result<(), Error> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let target = (i + 1) as i64;
        if version < target {
            conn.execute_batch(sql)?;
            conn.pragma_update(None, "user_version", target)?;
            info!("Applied DB migration v{target}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::Db;

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
}

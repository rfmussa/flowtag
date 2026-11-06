#[macro_use] extern crate log;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use anyhow::Error;
use rusqlite::Connection;

pub mod migrations;
pub mod models;
pub mod validate;
pub mod tags;
pub mod tracks;
pub mod flush;
pub mod scan;

pub use models::{DbFolder, DbGroup, DbTag, DbTrack, ScannedTrack};
pub use validate::FILE_TAG_SEPARATOR;

/// Name of the custom tag field written into audio files
pub const FILE_TAG_FIELD: &str = "TAGS";

/// Handle to the OneTagger library database.
///
/// One instance (with its internal connection mutex) is shared by the socket
/// handlers; the background flusher opens its own `Db` on the same path.
pub struct Db {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl Db {
    /// Open (creating if needed) and migrate the database at `path`
    pub fn open(path: impl AsRef<Path>) -> Result<Db, Error> {
        let conn = Connection::open(path.as_ref())?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrations::migrate(&conn)?;
        Ok(Db {
            conn: Mutex::new(conn),
            path: path.as_ref().to_owned(),
        })
    }

    /// Lock the underlying connection. A poisoned mutex (a panic while holding the
    /// lock) is recovered rather than propagated: the SQLite connection itself stays
    /// valid, and panicking every later caller would turn one bug into a dead app.
    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| {
            error!("DB mutex was poisoned by an earlier panic; recovering");
            poisoned.into_inner()
        })
    }

    /// Path this database was opened at
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Current unix time in seconds
pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;

    /// In-tempdir DB for tests
    pub fn test_db() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("test.db")).unwrap();
        (dir, db)
    }

    /// Insert a folder + a track row directly, returning the track id
    pub fn insert_track(db: &Db, path: &str) -> i64 {
        let conn = db.conn();
        conn.execute("INSERT OR IGNORE INTO folders (id, path) VALUES (1, '/test-root')", []).unwrap();
        conn.execute(
            "INSERT INTO tracks (folder_id, path, format, title, mtime, size) VALUES (1, ?1, 'mp3', 'Test', 0, 0)",
            [path],
        ).unwrap();
        conn.last_insert_rowid()
    }
}

use anyhow::Error;
use rusqlite::params;

use crate::Db;

/// Max flush retries before a track is left in the error state
pub const MAX_FLUSH_ATTEMPTS: i64 = 5;

/// A track due to have its tags written to the file
#[derive(Debug, Clone)]
pub struct FlushJob {
    pub track_id: i64,
    pub path: String,
    pub format: String,
    /// Tag names in display order
    pub tag_names: Vec<String>,
}

/// Tracks whose tags changed at least `debounce_secs` ago (or with `debounce_secs` = 0, all
/// dirty tracks), not missing, under the attempt cap, oldest first.
pub fn due_for_flush(db: &Db, now: i64, debounce_secs: i64, limit: usize) -> Result<Vec<FlushJob>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, path, format FROM tracks
         WHERE needs_flush = 1 AND missing = 0
           AND tag_updated_at <= ?1 AND flush_attempts < ?2
         ORDER BY tag_updated_at LIMIT ?3",
    )?;
    let rows = stmt
        .query_map(params![now - debounce_secs, MAX_FLUSH_ATTEMPTS, limit as i64], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut jobs = Vec::with_capacity(rows.len());
    let mut tag_stmt = conn.prepare(
        "SELECT t.name FROM track_tags tt JOIN tags t ON t.id = tt.tag_id
         WHERE tt.track_id = ?1 ORDER BY tt.position, t.id",
    )?;
    for (track_id, path, format) in rows {
        let tag_names = tag_stmt
            .query_map([track_id], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        jobs.push(FlushJob { track_id, path, format, tag_names });
    }
    Ok(jobs)
}

/// Record a successful flush: clear the dirty flag and store the file's post-write
/// stats so the next scan doesn't treat our own write as an external change.
pub fn mark_flushed(db: &Db, track_id: i64, mtime: i64, size: i64) -> Result<(), Error> {
    db.conn().execute(
        "UPDATE tracks SET needs_flush = 0, flush_attempts = 0, last_flush_error = NULL,
                           mtime = ?2, size = ?3
         WHERE id = ?1",
        params![track_id, mtime, size],
    )?;
    Ok(())
}

/// Record a failed flush attempt (track stays dirty, attempts incremented)
pub fn mark_flush_error(db: &Db, track_id: i64, error: &str) -> Result<(), Error> {
    db.conn().execute(
        "UPDATE tracks SET flush_attempts = flush_attempts + 1, last_flush_error = ?2 WHERE id = ?1",
        params![track_id, error],
    )?;
    Ok(())
}

/// Pending count + current errors, for the UI status indicator
pub fn flush_status(db: &Db) -> Result<(i64, Vec<(i64, String)>), Error> {
    let conn = db.conn();
    let pending: i64 = conn.query_row("SELECT COUNT(*) FROM tracks WHERE needs_flush = 1", [], |r| r.get(0))?;
    let mut stmt = conn.prepare(
        "SELECT id, last_flush_error FROM tracks WHERE last_flush_error IS NOT NULL LIMIT 100",
    )?;
    let errors = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok((pending, errors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::add_tag_to_track;
    use crate::testutil::{insert_track, test_db};

    #[test]
    fn debounce_and_ordering() {
        let (_d, db) = test_db();
        let t1 = insert_track(&db, "/test-root/a.mp3");
        let t2 = insert_track(&db, "/test-root/b.mp3");
        add_tag_to_track(&db, t1, "x").unwrap();
        add_tag_to_track(&db, t2, "y").unwrap();
        // Make t1 older than the debounce window, t2 fresh
        let now = crate::now();
        db.conn().execute("UPDATE tracks SET tag_updated_at = ?2 WHERE id = ?1", params![t1, now - 10]).unwrap();

        let due = due_for_flush(&db, now, 5, 16).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].track_id, t1);
        assert_eq!(due[0].tag_names, vec!["x"]);

        // With no debounce both are due, oldest first
        let due = due_for_flush(&db, now, 0, 16).unwrap();
        assert_eq!(due.len(), 2);
        assert_eq!(due[0].track_id, t1);
    }

    #[test]
    fn flush_success_clears_and_updates_stats() {
        let (_d, db) = test_db();
        let t = insert_track(&db, "/test-root/a.mp3");
        add_tag_to_track(&db, t, "x").unwrap();
        mark_flushed(&db, t, 123, 456).unwrap();
        let (pending, errors) = flush_status(&db).unwrap();
        assert_eq!(pending, 0);
        assert!(errors.is_empty());
        let (mtime, size): (i64, i64) = db.conn()
            .query_row("SELECT mtime, size FROM tracks WHERE id = ?1", [t], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!((mtime, size), (123, 456));
    }

    #[test]
    fn flush_errors_capped() {
        let (_d, db) = test_db();
        let t = insert_track(&db, "/test-root/a.mp3");
        add_tag_to_track(&db, t, "x").unwrap();
        db.conn().execute("UPDATE tracks SET tag_updated_at = 0", []).unwrap();
        for _ in 0..MAX_FLUSH_ATTEMPTS {
            assert_eq!(due_for_flush(&db, 100, 5, 16).unwrap().len(), 1);
            mark_flush_error(&db, t, "io error").unwrap();
        }
        // Capped out: no longer due, error visible in status
        assert!(due_for_flush(&db, 100, 5, 16).unwrap().is_empty());
        let (pending, errors) = flush_status(&db).unwrap();
        assert_eq!(pending, 1);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].1, "io error");
    }
}

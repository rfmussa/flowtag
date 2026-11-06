use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::time::Duration;
use anyhow::Error;
use rusqlite::params;
use serde::Serialize;
use onetagger_tag::{Tag, TagSeparators};

use crate::validate::FILE_TAG_SEPARATOR;
use crate::{Db, FILE_TAG_FIELD};

/// Max flush retries before a track is left in the error state
pub const MAX_FLUSH_ATTEMPTS: i64 = 5;

/// Base backoff after a failed flush attempt; doubles per attempt (30s, 60s, 120s, ...)
pub const FLUSH_BACKOFF_BASE_SECS: i64 = 30;

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
           AND next_flush_attempt_at <= ?4
         ORDER BY tag_updated_at LIMIT ?3",
    )?;
    let rows = stmt
        .query_map(params![now - debounce_secs, MAX_FLUSH_ATTEMPTS, limit as i64, now], |r| {
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
                           next_flush_attempt_at = 0, mtime = ?2, size = ?3
         WHERE id = ?1",
        params![track_id, mtime, size],
    )?;
    Ok(())
}

/// Record a failed flush attempt: the track stays dirty, attempts are incremented,
/// and the next retry is scheduled with exponential backoff (30s, 60s, 120s, ...).
pub fn mark_flush_error(db: &Db, track_id: i64, error: &str) -> Result<(), Error> {
    db.conn().execute(
        "UPDATE tracks SET flush_attempts = flush_attempts + 1,
                           last_flush_error = ?2,
                           next_flush_attempt_at = ?3 + ?4 * (1 << MIN(flush_attempts, 10))
         WHERE id = ?1",
        params![track_id, error, crate::now(), FLUSH_BACKOFF_BASE_SECS],
    )?;
    Ok(())
}

/// Seconds a track's tags must be stable before it is flushed
pub const FLUSH_DEBOUNCE_SECS: i64 = 5;
/// Max tracks flushed per pass
pub const FLUSH_BATCH: usize = 16;
/// Worker tick when nothing wakes it
pub const FLUSH_TICK: Duration = Duration::from_secs(2);

/// Result of flushing one track, broadcast to the UI
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlushEvent {
    pub track_id: i64,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Write a job's tag names into its file's TAGS field and return the file's
/// post-write (mtime, size). An empty tag list removes the field.
fn write_tags_to_file(job: &FlushJob) -> Result<(i64, i64), Error> {
    let path = Path::new(&job.path);
    // allow_new: files without any existing tag chunk get one created
    let mut tag_wrap = Tag::load_file(path, true)?;
    tag_wrap.set_separators(&TagSeparators {
        id3: FILE_TAG_SEPARATOR.to_string(),
        vorbis: None,
        mp4: FILE_TAG_SEPARATOR.to_string(),
    });
    let tag = tag_wrap.tag_mut();
    if job.tag_names.is_empty() {
        tag.remove_raw(FILE_TAG_FIELD);
    } else {
        tag.set_raw(FILE_TAG_FIELD, job.tag_names.clone(), true);
    }
    tag.save_file(path)?;

    let meta = std::fs::metadata(path)?;
    let mtime = meta
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok((mtime, meta.len() as i64))
}

/// One flush pass: write every due track (up to FLUSH_BATCH), except those the
/// caller asks to skip (e.g. the file currently playing). Skipped tracks stay
/// due; failures are recorded with backoff. Returns one event per attempt.
pub fn flush_pass(db: &Db, debounce_secs: i64, skip: &dyn Fn(&str) -> bool) -> Result<Vec<FlushEvent>, Error> {
    let jobs = due_for_flush(db, crate::now(), debounce_secs, FLUSH_BATCH)?;
    let mut events = Vec::with_capacity(jobs.len());
    for job in jobs {
        if skip(&job.path) {
            debug!("Flush: skipping in-use file {}", job.path);
            continue;
        }
        match write_tags_to_file(&job) {
            Ok((mtime, size)) => {
                mark_flushed(db, job.track_id, mtime, size)?;
                info!("Flushed tags to {}", job.path);
                events.push(FlushEvent { track_id: job.track_id, ok: true, error: None });
            }
            Err(e) => {
                warn!("Flush failed for {}: {e}", job.path);
                mark_flush_error(db, job.track_id, &e.to_string())?;
                events.push(FlushEvent { track_id: job.track_id, ok: false, error: Some(e.to_string()) });
            }
        }
    }
    Ok(events)
}

/// Make already-dirty tracks due immediately (never dirties a clean track)
pub fn expedite(db: &Db, track_id: Option<i64>) -> Result<(), Error> {
    match track_id {
        Some(id) => db.conn().execute(
            "UPDATE tracks SET tag_updated_at = 0 WHERE needs_flush = 1 AND id = ?1",
            [id],
        )?,
        None => db.conn().execute("UPDATE tracks SET tag_updated_at = 0 WHERE needs_flush = 1", [])?,
    };
    Ok(())
}

/// Reset attempts/backoff for errored tracks so the worker retries them
pub fn retry_errors(db: &Db) -> Result<(), Error> {
    db.conn().execute(
        "UPDATE tracks SET flush_attempts = 0, next_flush_attempt_at = 0, tag_updated_at = 0
         WHERE needs_flush = 1 AND last_flush_error IS NOT NULL",
        [],
    )?;
    Ok(())
}

/// Background flusher: an OS thread with its own DB connection that drains the
/// dirty queue every FLUSH_TICK or when woken through the returned sender.
pub struct FlushWorker;

impl FlushWorker {
    /// Spawn the worker. `on_event` is called for every flush attempt (from the
    /// worker thread); `skip` protects files that must not be written right now.
    pub fn spawn(
        db_path: PathBuf,
        on_event: impl Fn(FlushEvent) + Send + 'static,
        skip: impl Fn(&str) -> bool + Send + 'static,
    ) -> Result<Sender<()>, Error> {
        // Open eagerly so a broken DB path fails at startup, not silently in the thread
        let db = Db::open(&db_path)?;
        let (wake_tx, wake_rx) = channel::<()>();
        std::thread::Builder::new()
            .name("flush-worker".to_string())
            .spawn(move || {
                info!("Flush worker started");
                loop {
                    match wake_rx.recv_timeout(FLUSH_TICK) {
                        // Wake or tick: run a pass below
                        Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        // All senders gone: the app is shutting down
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    match flush_pass(&db, FLUSH_DEBOUNCE_SECS, &skip) {
                        Ok(events) => {
                            for event in events {
                                on_event(event);
                            }
                        }
                        Err(e) => error!("Flush pass failed: {e}"),
                    }
                }
                info!("Flush worker stopped");
            })?;
        Ok(wake_tx)
    }
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
    fn flush_error_backoff() {
        let (_d, db) = test_db();
        let t = insert_track(&db, "/test-root/a.mp3");
        add_tag_to_track(&db, t, "x").unwrap();
        let now = crate::now();
        db.conn().execute("UPDATE tracks SET tag_updated_at = ?1", [now - 100]).unwrap();
        assert_eq!(due_for_flush(&db, now, 5, 16).unwrap().len(), 1);
        // After a failure the track is NOT retried immediately...
        mark_flush_error(&db, t, "transient lock").unwrap();
        assert!(due_for_flush(&db, now, 5, 16).unwrap().is_empty());
        assert!(due_for_flush(&db, now + 5, 5, 16).unwrap().is_empty());
        // ...but becomes due again after the first backoff window (30s)
        assert_eq!(due_for_flush(&db, now + 31, 5, 16).unwrap().len(), 1);
        // Second failure doubles the wait (60s from the failure)
        mark_flush_error(&db, t, "transient lock").unwrap();
        assert!(due_for_flush(&db, now + 59, 5, 16).unwrap().is_empty());
        assert_eq!(due_for_flush(&db, now + 61, 5, 16).unwrap().len(), 1);
    }

    #[test]
    fn flush_errors_capped() {
        let (_d, db) = test_db();
        let t = insert_track(&db, "/test-root/a.mp3");
        add_tag_to_track(&db, t, "x").unwrap();
        db.conn().execute("UPDATE tracks SET tag_updated_at = 0", []).unwrap();
        // Query far enough in the future that backoff windows have always elapsed
        let future = crate::now() + 1_000_000;
        for _ in 0..MAX_FLUSH_ATTEMPTS {
            assert_eq!(due_for_flush(&db, future, 5, 16).unwrap().len(), 1);
            mark_flush_error(&db, t, "io error").unwrap();
        }
        // Capped out: no longer due, error visible in status
        assert!(due_for_flush(&db, future, 5, 16).unwrap().is_empty());
        let (pending, errors) = flush_status(&db).unwrap();
        assert_eq!(pending, 1);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].1, "io error");
    }
}

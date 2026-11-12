//! Staging: the working set of tracks the user is tagging right now.
//! Staged tracks are committed as "done" in a later step; skip marks a
//! track as listened-to with nothing to add. Local only: nothing here
//! touches files, `needs_flush` or Tangle.

use anyhow::Error;
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{now, Db};

/// Append one row to the edit log. Every kind is a QuickTag user edit; scan,
/// flush, comment export and Tangle sync never log. Callers log inside the
/// transaction/lock that performs the mutation so log order matches mutation order.
/// Kinds: tag_added, tag_removed, skip_set, skip_cleared, staged, unstaged,
/// committed, commit_reverted, tag_created, tag_renamed, tag_deleted.
pub fn log_event(c: &Connection, track_id: Option<i64>, kind: &str, data: &Value) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO edit_log (at, track_id, kind, data) VALUES (?1, ?2, ?3, ?4)",
        params![now(), track_id, kind, data.to_string()],
    )?;
    Ok(())
}

/// Log without letting a log failure fail the mutation it describes
pub(crate) fn log_or_warn(c: &Connection, track_id: Option<i64>, kind: &str, data: &Value) {
    if let Err(e) = log_event(c, track_id, kind, data) {
        log::warn!("edit_log write failed ({kind}): {e}");
    }
}

/// One staged track with its readiness (ready = skipped or has at least one tag)
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StageRow {
    pub track_id: i64,
    pub skip: bool,
    pub ready: bool,
}

/// Stage tracks (done tracks may be staged too). Returns how many were newly staged.
pub fn stage_tracks(db: &Db, ids: &[i64]) -> Result<usize, Error> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let at = now();
    let mut added = 0;
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO staged (track_id, staged_at)
             SELECT id, ?2 FROM tracks WHERE id = ?1",
        )?;
        for id in ids {
            let n = stmt.execute(params![id, at])?;
            if n > 0 {
                log_or_warn(&tx, Some(*id), "staged", &json!({}));
            }
            added += n;
        }
    }
    tx.commit()?;
    Ok(added)
}

/// Remove tracks from the stage. Returns how many were actually staged.
pub fn unstage_tracks(db: &Db, ids: &[i64]) -> Result<usize, Error> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let mut removed = 0;
    {
        let mut stmt = tx.prepare("DELETE FROM staged WHERE track_id = ?1")?;
        for id in ids {
            let n = stmt.execute([id])?;
            if n > 0 {
                log_or_warn(&tx, Some(*id), "unstaged", &json!({}));
            }
            removed += n;
        }
    }
    tx.commit()?;
    Ok(removed)
}

/// Flip a track's skip flag, staging it first when needed. Returns the new skip value.
pub fn toggle_skip(db: &Db, track_id: i64) -> Result<bool, Error> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let auto_staged = tx.execute(
        "INSERT OR IGNORE INTO staged (track_id, staged_at)
         SELECT id, ?2 FROM tracks WHERE id = ?1",
        params![track_id, now()],
    )?;
    if auto_staged > 0 {
        log_or_warn(&tx, Some(track_id), "staged", &json!({}));
    }
    tx.execute("UPDATE staged SET skip = 1 - skip WHERE track_id = ?1", [track_id])?;
    let skip: i64 = tx
        .query_row("SELECT skip FROM staged WHERE track_id = ?1", [track_id], |r| r.get(0))
        .map_err(|_| anyhow::anyhow!("Track {track_id} not found"))?;
    log_or_warn(&tx, Some(track_id), if skip != 0 { "skip_set" } else { "skip_cleared" }, &json!({}));
    tx.commit()?;
    Ok(skip != 0)
}

/// Staged tracks in stage order (staged_at, then path)
pub fn stage_list(db: &Db) -> Result<Vec<StageRow>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT s.track_id, s.skip,
                s.skip = 1 OR EXISTS (SELECT 1 FROM track_tags tt WHERE tt.track_id = s.track_id)
         FROM staged s JOIN tracks t ON t.id = s.track_id
         ORDER BY s.staged_at, t.path",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(StageRow { track_id: r.get(0)?, skip: r.get::<_, i64>(1)? != 0, ready: r.get::<_, i64>(2)? != 0 })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// (ready, total) over the stage
pub fn stage_counts(db: &Db) -> Result<(usize, usize), Error> {
    let rows = stage_list(db)?;
    let ready = rows.iter().filter(|r| r.ready).count();
    Ok((ready, rows.len()))
}

/// Present tracks marked done, library-wide
pub fn library_done_count(db: &Db) -> Result<usize, Error> {
    let n: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM tracks WHERE missing = 0 AND done_at IS NOT NULL",
        [],
        |r| r.get(0),
    )?;
    Ok(n as usize)
}

/// (folder_id, done, total) per folder; only present tracks (missing = 0) count
pub fn folder_done_counts(db: &Db) -> Result<Vec<(i64, usize, usize)>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT folder_id, SUM(done_at IS NOT NULL), COUNT(*)
         FROM tracks WHERE missing = 0 GROUP BY folder_id ORDER BY folder_id",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as usize, r.get::<_, i64>(2)? as usize)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// A commit: a batch of staged tracks marked done together
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitRow {
    pub id: i64,
    pub created_at: i64,
    pub message: String,
    pub track_count: i64,
}

/// Commit the given staged tracks as done, in one transaction. Ids that are not
/// staged are ignored; `track_count` is the number actually committed. Errors
/// when nothing would be committed.
pub fn commit_stage(db: &Db, track_ids: &[i64], message: &str) -> Result<CommitRow, Error> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let at = now();
    tx.execute("INSERT INTO commits (created_at, message, track_count) VALUES (?1, ?2, 0)", params![at, message])?;
    let commit_id = tx.last_insert_rowid();
    let mut committed: i64 = 0;
    {
        let mut unstage = tx.prepare("DELETE FROM staged WHERE track_id = ?1")?;
        let mut mark = tx.prepare("UPDATE tracks SET done_at = ?2, done_commit = ?3 WHERE id = ?1")?;
        for id in track_ids {
            if unstage.execute([id])? > 0 {
                mark.execute(params![id, at, commit_id])?;
                log_or_warn(&tx, Some(*id), "committed", &json!({ "commitId": commit_id }));
                committed += 1;
            }
        }
    }
    if committed == 0 {
        // dropping the transaction rolls the empty commit row back
        return Err(anyhow::anyhow!("Nothing to commit"));
    }
    tx.execute("UPDATE commits SET track_count = ?2 WHERE id = ?1", params![commit_id, committed])?;
    tx.commit()?;
    Ok(CommitRow { id: commit_id, created_at: at, message: message.to_string(), track_count: committed })
}

/// Undo a commit: its tracks lose done status and return to the stage (skip
/// cleared), and the commit row is deleted. Tags are never touched. Returns the
/// track count; an unknown commit id changes nothing and returns 0.
pub fn revert_commit(db: &Db, commit_id: i64) -> Result<usize, Error> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let restaged = tx.execute(
        "INSERT OR REPLACE INTO staged (track_id, staged_at, skip)
         SELECT id, ?2, 0 FROM tracks WHERE done_commit = ?1",
        params![commit_id, now()],
    )?;
    tx.execute(
        "UPDATE tracks SET done_at = NULL, done_commit = NULL WHERE done_commit = ?1",
        [commit_id],
    )?;
    let deleted = tx.execute("DELETE FROM commits WHERE id = ?1", [commit_id])?;
    if deleted > 0 {
        log_or_warn(&tx, None, "commit_reverted", &json!({ "commitId": commit_id, "count": restaged }));
    }
    tx.commit()?;
    Ok(restaged)
}

/// Commits, newest first
pub fn list_commits(db: &Db, limit: usize) -> Result<Vec<CommitRow>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, created_at, message, track_count FROM commits ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([limit as i64], |r| {
            Ok(CommitRow { id: r.get(0)?, created_at: r.get(1)?, message: r.get(2)?, track_count: r.get(3)? })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Journal numbers for one day
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TodayStats {
    pub committed: usize,
    pub tags_created: usize,
    pub active_secs: i64,
}

/// Longest gap between two edits that still counts as continuous work
const ACTIVE_GAP_CAP_SECS: i64 = 300;

/// Stats over edit_log entries with `day_start <= at <= now`. Active time is
/// the sum of min(gap, 300s) between consecutive entries (0 for fewer than two).
pub fn today_stats(db: &Db, day_start: i64, now: i64) -> Result<TodayStats, Error> {
    let conn = db.conn();
    let count = |kind: &str| -> Result<usize, Error> {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM edit_log WHERE kind = ?1 AND at >= ?2 AND at <= ?3",
            params![kind, day_start, now],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    };
    let mut stmt = conn.prepare("SELECT at FROM edit_log WHERE at >= ?1 AND at <= ?2 ORDER BY at, id")?;
    let times = stmt.query_map([day_start, now], |r| r.get::<_, i64>(0))?.collect::<Result<Vec<_>, _>>()?;
    let active_secs = times.windows(2).map(|w| (w[1] - w[0]).min(ACTIVE_GAP_CAP_SECS)).sum();
    Ok(TodayStats { committed: count("committed")?, tags_created: count("tag_created")?, active_secs })
}

/// (local YYYY-MM-DD, tracks committed) for days that have commits, newest first
pub fn commit_days(db: &Db, limit: usize) -> Result<Vec<(String, usize)>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT date(created_at, 'unixepoch', 'localtime') AS day, SUM(track_count)
         FROM commits GROUP BY day ORDER BY day DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([limit as i64], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// One edit_log entry for the Journal trail
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrailRow {
    pub id: i64,
    pub at: i64,
    pub track_id: Option<i64>,
    pub track_title: Option<String>,
    pub kind: String,
    pub data: String,
}

/// Edit log newest first; pass the last row's id as `before_id` for the next page
pub fn trail(db: &Db, before_id: Option<i64>, limit: usize) -> Result<Vec<TrailRow>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT e.id, e.at, e.track_id, t.title, e.kind, e.data
         FROM edit_log e LEFT JOIN tracks t ON t.id = e.track_id
         WHERE (?1 IS NULL OR e.id < ?1)
         ORDER BY e.id DESC LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![before_id, limit as i64], |r| {
            Ok(TrailRow {
                id: r.get(0)?,
                at: r.get(1)?,
                track_id: r.get(2)?,
                track_title: r.get(3)?,
                kind: r.get(4)?,
                data: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{insert_track, test_db};

    #[test]
    fn v11_creates_staging_tables() {
        let (_dir, db) = test_db();
        let conn = db.conn();
        for table in ["staged", "commits", "edit_log"] {
            let exists = conn
                .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1").unwrap()
                .exists([table]).unwrap();
            assert!(exists, "{table} missing after v11");
        }
        conn.prepare("SELECT done_at, done_commit FROM tracks").unwrap();
    }

    #[test]
    fn stage_unstage_idempotent() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        assert_eq!(stage_tracks(&db, &[a]).unwrap(), 1);
        assert_eq!(stage_tracks(&db, &[a]).unwrap(), 0);
        assert_eq!(stage_list(&db).unwrap().len(), 1);
        assert_eq!(unstage_tracks(&db, &[a]).unwrap(), 1);
        assert_eq!(unstage_tracks(&db, &[a]).unwrap(), 0);
        assert!(stage_list(&db).unwrap().is_empty());
    }

    #[test]
    fn toggle_skip_autostages() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        assert!(toggle_skip(&db, a).unwrap());
        let rows = stage_list(&db).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].skip && rows[0].ready);
        assert!(!toggle_skip(&db, a).unwrap());
        let rows = stage_list(&db).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].skip && !rows[0].ready);
    }

    #[test]
    fn ready_means_skip_or_tags() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        let b = insert_track(&db, "/test-root/b.mp3");
        stage_tracks(&db, &[a, b]).unwrap();
        assert_eq!(stage_counts(&db).unwrap(), (0, 2));
        crate::tags::add_tag_to_track(&db, a, "dark").unwrap();
        let rows = stage_list(&db).unwrap();
        assert!(rows.iter().find(|r| r.track_id == a).unwrap().ready);
        assert!(!rows.iter().find(|r| r.track_id == b).unwrap().ready);
        assert_eq!(stage_counts(&db).unwrap(), (1, 2));
    }

    #[test]
    fn counts_and_folder_counts() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        let b = insert_track(&db, "/test-root/b.mp3");
        let _c = insert_track(&db, "/test-root/c.mp3");
        let m = insert_track(&db, "/test-root/gone.mp3");
        {
            let conn = db.conn();
            conn.execute("UPDATE tracks SET done_at = 100 WHERE id IN (?1, ?2)", [a, b]).unwrap();
            // R1: a missing track counts toward neither total nor done
            conn.execute("UPDATE tracks SET missing = 1, done_at = 100 WHERE id = ?1", [m]).unwrap();
        }
        assert_eq!(library_done_count(&db).unwrap(), 2);
        assert_eq!(folder_done_counts(&db).unwrap(), vec![(1, 2, 3)]);
    }

    #[test]
    fn done_tracks_can_be_staged() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        db.conn().execute("UPDATE tracks SET done_at = 100 WHERE id = ?1", [a]).unwrap();
        assert_eq!(stage_tracks(&db, &[a]).unwrap(), 1);
    }

    #[test]
    fn stage_list_orders_by_staged_at_then_path() {
        let (_dir, db) = test_db();
        let b = insert_track(&db, "/test-root/b.mp3");
        let a = insert_track(&db, "/test-root/a.mp3");
        let c = insert_track(&db, "/test-root/c.mp3");
        stage_tracks(&db, &[b, a, c]).unwrap();
        db.conn().execute("UPDATE staged SET staged_at = 50 WHERE track_id = ?1", [c]).unwrap();
        let ids: Vec<i64> = stage_list(&db).unwrap().iter().map(|r| r.track_id).collect();
        assert_eq!(ids, vec![c, a, b]);
    }

    fn done_state(db: &Db, id: i64) -> (Option<i64>, Option<i64>) {
        db.conn()
            .query_row("SELECT done_at, done_commit FROM tracks WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
    }

    #[test]
    fn commit_marks_done_and_unstages() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        let b = insert_track(&db, "/test-root/b.mp3");
        let c = insert_track(&db, "/test-root/c.mp3");
        stage_tracks(&db, &[a, b, c]).unwrap();
        let commit = commit_stage(&db, &[a, b], "first batch").unwrap();
        assert_eq!(commit.track_count, 2);
        assert_eq!(commit.message, "first batch");
        for id in [a, b] {
            let (at, done_commit) = done_state(&db, id);
            assert!(at.is_some());
            assert_eq!(done_commit, Some(commit.id));
        }
        assert_eq!(done_state(&db, c), (None, None));
        let ids: Vec<i64> = stage_list(&db).unwrap().iter().map(|r| r.track_id).collect();
        assert_eq!(ids, vec![c]);
        let stored: i64 = db.conn().query_row("SELECT track_count FROM commits WHERE id = ?1", [commit.id], |r| r.get(0)).unwrap();
        assert_eq!(stored, 2);
    }

    #[test]
    fn commit_ignores_unstaged_ids() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        let b = insert_track(&db, "/test-root/b.mp3");
        stage_tracks(&db, &[a]).unwrap();
        let commit = commit_stage(&db, &[a, b], "").unwrap();
        assert_eq!(commit.track_count, 1);
        assert_eq!(done_state(&db, b), (None, None));
    }

    #[test]
    fn commit_with_no_ticked_ids_is_an_error() {
        let (_dir, db) = test_db();
        assert!(commit_stage(&db, &[], "x").is_err());
    }

    #[test]
    fn recommit_of_done_track_updates_commit_id() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        stage_tracks(&db, &[a]).unwrap();
        let first = commit_stage(&db, &[a], "one").unwrap();
        db.conn().execute("UPDATE tracks SET done_at = 1 WHERE id = ?1", [a]).unwrap();
        stage_tracks(&db, &[a]).unwrap();
        let second = commit_stage(&db, &[a], "two").unwrap();
        assert_ne!(first.id, second.id);
        let (at, done_commit) = done_state(&db, a);
        assert!(at.unwrap() > 1);
        assert_eq!(done_commit, Some(second.id));
    }

    #[test]
    fn revert_restages_and_clears_done() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        let b = insert_track(&db, "/test-root/b.mp3");
        stage_tracks(&db, &[a, b]).unwrap();
        toggle_skip(&db, a).unwrap();
        let commit = commit_stage(&db, &[a, b], "m").unwrap();
        assert_eq!(revert_commit(&db, commit.id).unwrap(), 2);
        assert_eq!(done_state(&db, a), (None, None));
        assert_eq!(done_state(&db, b), (None, None));
        let rows = stage_list(&db).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| !r.skip));
        assert!(list_commits(&db, 10).unwrap().is_empty());
    }

    #[test]
    fn revert_unknown_commit_is_a_noop() {
        let (_dir, db) = test_db();
        assert_eq!(revert_commit(&db, 999).unwrap(), 0);
    }

    #[test]
    fn revert_leaves_tags_untouched() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        stage_tracks(&db, &[a]).unwrap();
        let commit = commit_stage(&db, &[a], "m").unwrap();
        crate::tags::add_tag_to_track(&db, a, "dark").unwrap();
        revert_commit(&db, commit.id).unwrap();
        let n: i64 = db.conn().query_row("SELECT COUNT(*) FROM track_tags WHERE track_id = ?1", [a], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn list_commits_newest_first_with_limit() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        let b = insert_track(&db, "/test-root/b.mp3");
        stage_tracks(&db, &[a, b]).unwrap();
        let c1 = commit_stage(&db, &[a], "one").unwrap();
        let c2 = commit_stage(&db, &[b], "two").unwrap();
        let ids: Vec<i64> = list_commits(&db, 10).unwrap().iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![c2.id, c1.id]);
        assert_eq!(list_commits(&db, 1).unwrap().len(), 1);
    }

    fn log_kinds(db: &Db) -> Vec<String> {
        let conn = db.conn();
        let mut stmt = conn.prepare("SELECT kind FROM edit_log ORDER BY id").unwrap();
        let rows = stmt.query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
        rows
    }

    fn clear_log(db: &Db) {
        db.conn().execute("DELETE FROM edit_log", []).unwrap();
    }

    fn insert_log(db: &Db, at: i64, track_id: Option<i64>, kind: &str) {
        db.conn()
            .execute("INSERT INTO edit_log (at, track_id, kind) VALUES (?1, ?2, ?3)", params![at, track_id, kind])
            .unwrap();
    }

    #[test]
    fn stage_ops_write_log() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        let b = insert_track(&db, "/test-root/b.mp3");
        stage_tracks(&db, &[a, b]).unwrap();
        stage_tracks(&db, &[a]).unwrap(); // already staged: no row
        assert_eq!(log_kinds(&db), vec!["staged", "staged"]);
        clear_log(&db);
        unstage_tracks(&db, &[a, 999]).unwrap(); // 999 not staged: no row
        assert_eq!(log_kinds(&db), vec!["unstaged"]);
        clear_log(&db);
        toggle_skip(&db, a).unwrap(); // auto-stages, then skip set
        toggle_skip(&db, a).unwrap();
        assert_eq!(log_kinds(&db), vec!["staged", "skip_set", "skip_cleared"]);
    }

    #[test]
    fn commit_and_revert_write_log() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        let b = insert_track(&db, "/test-root/b.mp3");
        stage_tracks(&db, &[a, b]).unwrap();
        clear_log(&db);
        let commit = commit_stage(&db, &[a, b], "m").unwrap();
        assert_eq!(log_kinds(&db), vec!["committed", "committed"]);
        let data: String = db.conn().query_row("SELECT data FROM edit_log WHERE track_id = ?1", [a], |r| r.get(0)).unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&data).unwrap(), serde_json::json!({"commitId": commit.id}));
        clear_log(&db);
        revert_commit(&db, commit.id).unwrap();
        assert_eq!(log_kinds(&db), vec!["commit_reverted"]);
        let (track_id, data): (Option<i64>, String) = db
            .conn()
            .query_row("SELECT track_id, data FROM edit_log", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(track_id, None);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&data).unwrap(),
            serde_json::json!({"commitId": commit.id, "count": 2})
        );
        // failed / no-op operations log nothing
        clear_log(&db);
        assert!(commit_stage(&db, &[], "x").is_err());
        revert_commit(&db, 999).unwrap();
        assert!(log_kinds(&db).is_empty());
    }

    #[test]
    fn active_time_gap_cap_and_day_bounds() {
        let (_dir, db) = test_db();
        let t = 1_000_000;
        insert_log(&db, t - 86_400, None, "tag_added"); // yesterday: outside the window
        insert_log(&db, t, None, "tag_added");
        insert_log(&db, t + 100, None, "tag_added");
        insert_log(&db, t + 10_000, None, "tag_added");
        let stats = today_stats(&db, t - 10, t + 20_000).unwrap();
        assert_eq!(stats.active_secs, 100 + 300);
        // fewer than two entries in the window: nothing to measure
        assert_eq!(today_stats(&db, t + 5_000, t + 20_000).unwrap().active_secs, 0);
        assert_eq!(today_stats(&db, t + 20_000, t + 30_000).unwrap().active_secs, 0);
    }

    #[test]
    fn today_stats_counts_commits_and_created_tags() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        let b = insert_track(&db, "/test-root/b.mp3");
        stage_tracks(&db, &[a, b]).unwrap();
        commit_stage(&db, &[a, b], "m").unwrap();
        crate::tags::get_or_create_tag(&db, "dark", None).unwrap();
        let stats = today_stats(&db, 0, now() + 10).unwrap();
        assert_eq!(stats.committed, 2);
        assert_eq!(stats.tags_created, 1);
        assert_eq!(today_stats(&db, now() + 100, now() + 200).unwrap().committed, 0);
    }

    #[test]
    fn commit_days_buckets_only_commit_days() {
        let (_dir, db) = test_db();
        {
            let conn = db.conn();
            // 2023-11-15 12:00 UTC: same calendar day in any timezone for +1h
            const NOON: i64 = 1_700_049_600;
            for (id, at, n) in [(1, NOON, 3), (2, NOON + 3_600, 2), (3, NOON + 3 * 86_400, 4)] {
                conn.execute(
                    "INSERT INTO commits (id, created_at, message, track_count) VALUES (?1, ?2, '', ?3)",
                    params![id, at, n],
                )
                .unwrap();
            }
        }
        let days = commit_days(&db, 10).unwrap();
        assert_eq!(days.len(), 2, "days without commits are absent: {days:?}");
        assert!(days[0].0 > days[1].0, "newest first");
        assert_eq!(days[0].1, 4);
        assert_eq!(days[1].1, 5);
        assert_eq!(days[0].0.len(), 10);
        assert_eq!(commit_days(&db, 1).unwrap().len(), 1);
    }

    #[test]
    fn trail_pages_newest_first_with_titles() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        db.conn().execute("UPDATE tracks SET title = 'Alpha' WHERE id = ?1", [a]).unwrap();
        for i in 0..5 {
            insert_log(&db, 100 + i, if i % 2 == 0 { Some(a) } else { None }, "tag_added");
        }
        let first = trail(&db, None, 2).unwrap();
        assert_eq!(first.len(), 2);
        assert!(first[0].id > first[1].id);
        assert_eq!(first[0].track_title.as_deref(), Some("Alpha"));
        assert_eq!(first[1].track_title, None);
        let second = trail(&db, Some(first[1].id), 10).unwrap();
        assert_eq!(second.len(), 3);
        assert!(second[0].id < first[1].id);
        assert_eq!(second[0].kind, "tag_added");
        assert_eq!(second[0].data, "{}");
    }
}

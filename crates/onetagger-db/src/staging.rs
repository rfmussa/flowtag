//! Staging: the working set of tracks the user is tagging right now.
//! Staged tracks are committed as "done" in a later step; skip marks a
//! track as listened-to with nothing to add. Local only: nothing here
//! touches files, `needs_flush` or Tangle.

use anyhow::Error;
use rusqlite::params;
use serde::Serialize;

use crate::{now, Db};

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
            added += stmt.execute(params![id, at])?;
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
            removed += stmt.execute([id])?;
        }
    }
    tx.commit()?;
    Ok(removed)
}

/// Flip a track's skip flag, staging it first when needed. Returns the new skip value.
pub fn toggle_skip(db: &Db, track_id: i64) -> Result<bool, Error> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT OR IGNORE INTO staged (track_id, staged_at)
         SELECT id, ?2 FROM tracks WHERE id = ?1",
        params![track_id, now()],
    )?;
    tx.execute("UPDATE staged SET skip = 1 - skip WHERE track_id = ?1", [track_id])?;
    let skip: i64 = tx
        .query_row("SELECT skip FROM staged WHERE track_id = ?1", [track_id], |r| r.get(0))
        .map_err(|_| anyhow::anyhow!("Track {track_id} not found"))?;
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
    tx.execute("DELETE FROM commits WHERE id = ?1", [commit_id])?;
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
}

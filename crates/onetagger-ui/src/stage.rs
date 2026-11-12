//! Staging and journal websocket handlers: thin glue over `onetagger_db::staging`.

use anyhow::Error;
use serde_json::{json, Value};
use onetagger_db::{staging, Db};

use crate::socket::SocketContext;
use crate::tagflow::unix_now;

/// Commits shown in the Journal
const JOURNAL_COMMIT_LIMIT: usize = 30;
/// Commit-day rows shown in the Journal heatmap
const JOURNAL_DAY_LIMIT: usize = 365;
/// Trail rows per page
const TRAIL_PAGE_SIZE: usize = 100;

/// Unix seconds of local midnight today
fn local_day_start() -> i64 {
    use chrono::{Local, TimeZone};
    let midnight = Local::now().date_naive().and_hms_opt(0, 0, 0);
    midnight
        .and_then(|m| Local.from_local_datetime(&m).earliest())
        .map(|t| t.timestamp())
        .unwrap_or_else(|| unix_now() - unix_now() % 86_400)
}

/// The `stageState` message: counts, per-folder done counts and the stage list
pub(crate) fn state_message(db: &Db) -> Result<Value, Error> {
    let (ready, total) = staging::stage_counts(db)?;
    let folder_counts: Vec<Value> = staging::folder_done_counts(db)?
        .into_iter()
        .map(|(folder, done, total)| json!([folder, done, total]))
        .collect();
    Ok(json!({
        "action": "stageState",
        "ready": ready,
        "total": total,
        "libraryDone": staging::library_done_count(db)?,
        "folderCounts": folder_counts,
        "stage": staging::stage_list(db)?,
    }))
}

/// Broadcast `stageState` to every connection
pub(crate) fn broadcast_state(context: &SocketContext) -> Result<(), Error> {
    context.broadcast(state_message(&context.db)?)
}

pub(crate) fn stage_tracks(context: &SocketContext, ids: &[i64]) -> Result<(), Error> {
    staging::stage_tracks(&context.db, ids)?;
    broadcast_state(context)
}

pub(crate) fn unstage_tracks(context: &SocketContext, ids: &[i64]) -> Result<(), Error> {
    staging::unstage_tracks(&context.db, ids)?;
    broadcast_state(context)
}

pub(crate) fn stage_skip(context: &SocketContext, track_id: i64) -> Result<(), Error> {
    staging::toggle_skip(&context.db, track_id)?;
    broadcast_state(context)
}

/// Commit the given staged tracks as done; errors (e.g. nothing to commit) reach
/// the client through the socket's standard `error` reply
pub(crate) fn stage_commit(context: &SocketContext, ids: &[i64], message: &str) -> Result<(), Error> {
    let commit = staging::commit_stage(&context.db, ids, message)?;
    context.broadcast(json!({ "action": "stageCommitted", "commit": commit }))?;
    broadcast_state(context)
}

pub(crate) fn stage_revert(context: &SocketContext, commit_id: i64) -> Result<(), Error> {
    staging::revert_commit(&context.db, commit_id)?;
    broadcast_state(context)
}

/// Journal: today's numbers, commit days, one trail page and recent commits
pub(crate) fn journal_get(context: &SocketContext, before_id: Option<i64>) -> Result<(), Error> {
    let db = &context.db;
    let days: Vec<Value> = staging::commit_days(db, JOURNAL_DAY_LIMIT)?
        .into_iter()
        .map(|(date, count)| json!([date, count]))
        .collect();
    context.send(json!({
        "action": "journal",
        "today": staging::today_stats(db, local_day_start(), unix_now())?,
        "days": days,
        "trail": staging::trail(db, before_id, TRAIL_PAGE_SIZE)?,
        "commits": staging::list_commits(db, JOURNAL_COMMIT_LIMIT)?,
    }))
}

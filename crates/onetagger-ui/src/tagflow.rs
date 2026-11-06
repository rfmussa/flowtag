//! FlowTag websocket handlers: thin glue between socket actions and `onetagger-db`.

use anyhow::Error;
use serde_json::{json, Value};
use onetagger_db::{flush, scan, tags, tracks, Db};

use crate::sender::MessageSender;
use crate::socket::SocketContext;

/// Partial tag update as received from the client
pub(crate) struct TagPatch {
    pub name: Option<String>,
    pub color: Option<String>,
    pub group_id: Option<i64>,
    pub clear_group: Option<bool>,
    pub keybind: Option<Value>,
    pub clear_keybind: Option<bool>,
}

/// Send the `tagFolders` reply with the current folder list
fn send_folders(context: &SocketContext) -> Result<(), Error> {
    context.send(json!({
        "action": "tagFolders",
        "folders": tracks::list_folders(&context.db)?
    }))
}

/// Build the `tagPool` message (tags + groups)
fn pool_message(db: &Db) -> Result<Value, Error> {
    Ok(json!({
        "action": "tagPool",
        "tags": tags::list_tags(db)?,
        "groups": tags::list_groups(db)?
    }))
}

/// Build the `tagTracks` message
fn tracks_message(db: &Db) -> Result<Value, Error> {
    Ok(json!({
        "action": "tagTracks",
        "tracks": tracks::list_tracks(db)?
    }))
}

pub(crate) fn folder_add(context: &SocketContext, path: &str) -> Result<(), Error> {
    tracks::add_folder(&context.db, path)?;
    send_folders(context)
}

pub(crate) fn folder_remove(context: &SocketContext, id: i64) -> Result<(), Error> {
    tracks::remove_folder(&context.db, id)?;
    send_folders(context)
}

pub(crate) fn folder_list(context: &SocketContext) -> Result<(), Error> {
    send_folders(context)
}

/// Start a library scan on a blocking task; returns immediately
pub(crate) fn library_scan(context: &SocketContext, full: bool) {
    let sender = context.sender.clone();
    let db = context.db.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = run_scan(&db, &sender, full) {
            error!("Library scan failed: {e}");
            sender.send(json!({ "action": "error", "message": e.to_string() })).ok();
        }
    });
}

fn run_scan(db: &Db, sender: &MessageSender, full: bool) -> Result<(), Error> {
    let result = scan::scan(db, full, |done, total| {
        sender.send(json!({ "action": "tagScanProgress", "done": done, "total": total })).ok();
    })?;
    sender.send(json!({ "action": "tagScanDone", "result": result }))?;
    sender.send(pool_message(db)?)?;
    sender.send(tracks_message(db)?)?;
    Ok(())
}

pub(crate) fn tracks_load(context: &SocketContext) -> Result<(), Error> {
    context.send(tracks_message(&context.db)?)
}

pub(crate) fn pool_load(context: &SocketContext) -> Result<(), Error> {
    context.send(pool_message(&context.db)?)
}

pub(crate) fn tag_create(context: &SocketContext, name: &str, color: Option<&str>) -> Result<(), Error> {
    let (tag, _created) = tags::get_or_create_tag(&context.db, name, color)?;
    context.send(json!({ "action": "tagCreated", "tag": tag }))
}

pub(crate) fn tag_update(context: &SocketContext, id: i64, patch: TagPatch) -> Result<(), Error> {
    // clear_* wins over a value: Some(None) clears, Some(Some(v)) sets, None leaves untouched
    let group_id = match (patch.clear_group.unwrap_or(false), patch.group_id) {
        (true, _) => Some(None),
        (false, Some(g)) => Some(Some(g)),
        (false, None) => None,
    };
    let keybind = match (patch.clear_keybind.unwrap_or(false), patch.keybind) {
        (true, _) => Some(None),
        (false, Some(k)) => Some(Some(k)),
        (false, None) => None,
    };
    let tag = tags::update_tag(
        &context.db, id, patch.name.as_deref(), patch.color.as_deref(), group_id, keybind
    )?;
    context.send(json!({ "action": "tagUpdated", "tag": tag }))
}

pub(crate) fn tag_delete(context: &SocketContext, id: i64) -> Result<(), Error> {
    tags::delete_tag(&context.db, id)?;
    context.send(json!({ "action": "tagDeleted", "id": id }))?;
    pool_load(context)
}

pub(crate) fn group_create(context: &SocketContext, name: &str, color: &str) -> Result<(), Error> {
    tags::create_group(&context.db, name, color)?;
    pool_load(context)
}

pub(crate) fn group_update(
    context: &SocketContext, id: i64, name: Option<&str>, color: Option<&str>, position: Option<i64>
) -> Result<(), Error> {
    tags::update_group(&context.db, id, name, color, position)?;
    pool_load(context)
}

pub(crate) fn group_delete(context: &SocketContext, id: i64) -> Result<(), Error> {
    tags::delete_group(&context.db, id)?;
    pool_load(context)
}

pub(crate) fn track_add_tag(context: &SocketContext, track_id: i64, name: &str) -> Result<(), Error> {
    let (tag_ids, new_tag) = tags::add_tag_to_track(&context.db, track_id, name)?;
    context.send(json!({
        "action": "trackTagsChanged",
        "trackId": track_id,
        "tagIds": tag_ids,
        "newTag": new_tag
    }))
}

pub(crate) fn track_remove_tag(context: &SocketContext, track_id: i64, tag_id: i64) -> Result<(), Error> {
    let tag_ids = tags::remove_tag_from_track(&context.db, track_id, tag_id)?;
    context.send(json!({
        "action": "trackTagsChanged",
        "trackId": track_id,
        "tagIds": tag_ids
    }))
}

/// Expedite pending flushes (never dirties a clean track) and wake the worker
pub(crate) fn flush_hint(context: &SocketContext, track_id: Option<i64>) -> Result<(), Error> {
    flush::expedite(&context.db, track_id)?;
    context.flush_wake.send(()).ok();
    Ok(())
}

/// Reset errored tracks for another round of flush attempts
pub(crate) fn flush_retry(context: &SocketContext) -> Result<(), Error> {
    flush::retry_errors(&context.db)?;
    context.flush_wake.send(()).ok();
    flush_status_get(context)
}

/// Number of waveform bars sent to the UI
const WAVEFORM_BARS: usize = 180;
/// Stored bar scale: f32 (0..=1) quantized to a byte
const WAVEFORM_SCALE: f32 = 255.0;

fn waveform_message(track_id: i64, bars: &[u8]) -> Value {
    let bars: Vec<f32> = bars.iter().map(|b| *b as f32 / WAVEFORM_SCALE).collect();
    json!({ "action": "trackWaveform", "trackId": track_id, "bars": bars })
}

/// Waveform by track id: served from the DB cache, generated (and cached) otherwise
pub(crate) fn waveform_get(context: &SocketContext, track_id: i64) -> Result<(), Error> {
    if let Some(bars) = tracks::get_waveform(&context.db, track_id)? {
        return context.send(waveform_message(track_id, &bars));
    }
    let sender = context.sender.clone();
    let db = context.db.clone();
    tokio::task::spawn_blocking(move || {
        let result: Result<(), Error> = (|| {
            let path = tracks::get_track_path(&db, track_id)?;
            let source = onetagger_player::AudioSources::from_path(&path)?;
            let (rx, _cancel) = source.generate_waveform(WAVEFORM_BARS as i16)?;
            let mut bars: Vec<u8> = rx
                .into_iter()
                .map(|w| (w.clamp(0.0, 1.0) * WAVEFORM_SCALE) as u8)
                .collect();
            bars.resize(WAVEFORM_BARS, 0);
            tracks::set_waveform(&db, track_id, &bars)?;
            sender.send(waveform_message(track_id, &bars))?;
            Ok(())
        })();
        if let Err(e) = result {
            warn!("Waveform generation failed for track {track_id}: {e}");
        }
    });
    Ok(())
}

pub(crate) fn flush_status_get(context: &SocketContext) -> Result<(), Error> {
    let (pending, errors) = flush::flush_status(&context.db)?;
    let errors: Vec<Value> = errors.into_iter()
        .map(|(track_id, error)| json!({ "trackId": track_id, "error": error }))
        .collect();
    context.send(json!({ "action": "flushStatus", "pending": pending, "errors": errors }))
}

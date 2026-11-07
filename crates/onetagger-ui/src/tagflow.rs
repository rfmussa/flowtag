//! FlowTag websocket handlers: thin glue between socket actions and `onetagger-db`.

use std::sync::atomic::{AtomicI64, Ordering};
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
    send_folders(context)?;
    // Cascade removed tracks and tag links: refresh both lists
    context.send(tracks_message(&context.db)?)?;
    context.send(pool_message(&context.db)?)
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
    let changed = result.added + result.updated + result.removed > 0;
    sender.send(json!({ "action": "tagScanDone", "result": result }))?;
    // Resending 20k rows makes a no-op rescan feel slow; skip it when clean
    if changed || full {
        sender.send(pool_message(db)?)?;
        sender.send(tracks_message(db)?)?;
    }
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
    context.broadcast(json!({ "action": "tagCreated", "tag": tag }))
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
    context.broadcast(json!({ "action": "tagUpdated", "tag": tag }))
}

pub(crate) fn tag_delete(context: &SocketContext, id: i64) -> Result<(), Error> {
    tags::delete_tag(&context.db, id)?;
    context.broadcast(json!({ "action": "tagDeleted", "id": id }))?;
    pool_load(context)
}

pub(crate) fn group_create(context: &SocketContext, name: &str, color: &str) -> Result<(), Error> {
    tags::create_group(&context.db, name, color)?;
    context.broadcast(pool_message(&context.db)?)
}

pub(crate) fn group_update(
    context: &SocketContext, id: i64, name: Option<&str>, color: Option<&str>, position: Option<i64>
) -> Result<(), Error> {
    tags::update_group(&context.db, id, name, color, position)?;
    context.broadcast(pool_message(&context.db)?)
}

pub(crate) fn group_delete(context: &SocketContext, id: i64) -> Result<(), Error> {
    tags::delete_group(&context.db, id)?;
    context.broadcast(pool_message(&context.db)?)
}

pub(crate) fn track_add_tag(context: &SocketContext, track_id: i64, name: &str) -> Result<(), Error> {
    let (tag_ids, new_tag) = tags::add_tag_to_track(&context.db, track_id, name)?;
    context.broadcast(json!({
        "action": "trackTagsChanged",
        "trackId": track_id,
        "tagIds": tag_ids,
        "newTag": new_tag
    }))
}

pub(crate) fn track_remove_tag(context: &SocketContext, track_id: i64, tag_id: i64) -> Result<(), Error> {
    let tag_ids = tags::remove_tag_from_track(&context.db, track_id, tag_id)?;
    context.broadcast(json!({
        "action": "trackTagsChanged",
        "trackId": track_id,
        "tagIds": tag_ids
    }))
}

/// Bulk tag add (multi-select): one transaction, one compact echo
pub(crate) fn tracks_add_tag(context: &SocketContext, track_ids: &[i64], name: &str) -> Result<(), Error> {
    let (tag, created) = tags::add_tag_to_tracks(&context.db, track_ids, name)?;
    context.broadcast(json!({
        "action": "tracksTagsChanged",
        "trackIds": track_ids,
        "tagId": tag.id,
        "op": "add",
        "newTag": created.then_some(tag)
    }))
}

/// Bulk tag remove (multi-select)
pub(crate) fn tracks_remove_tag(context: &SocketContext, track_ids: &[i64], tag_id: i64) -> Result<(), Error> {
    tags::remove_tag_from_tracks(&context.db, track_ids, tag_id)?;
    context.broadcast(json!({
        "action": "tracksTagsChanged",
        "trackIds": track_ids,
        "tagId": tag_id,
        "op": "remove"
    }))
}

/// Track inspector: current metadata, read fresh from the file
pub(crate) fn track_meta_get(context: &SocketContext, track_id: i64) -> Result<(), Error> {
    let path = tracks::get_track_path(&context.db, track_id)?;
    let meta = onetagger_db::meta::read_track_meta(&path)?;
    context.send(json!({
        "action": "trackMeta",
        "trackId": track_id,
        "meta": meta
    }))
}

/// Track inspector: write metadata to the file and the DB row. The client
/// stops playback before saving; this guard catches anything that slips past.
pub(crate) fn track_meta_set(context: &SocketContext, track_id: i64, meta: &onetagger_db::meta::TrackMeta) -> Result<(), Error> {
    let path = tracks::get_track_path(&context.db, track_id)?;
    if crate::socket::is_currently_playing(&path) {
        return Err(anyhow::anyhow!("Stop playback before editing this file's tags"));
    }
    let saved = onetagger_db::meta::write_track_meta(&context.db, track_id, meta)?;
    context.broadcast(json!({
        "action": "trackMetaChanged",
        "trackId": track_id,
        "meta": saved
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

/// Export tags into Rekordbox-visible fields on a blocking task.
/// Only "comment" (trailing hashtag block) is implemented; "grouping" and
/// "rekordboxXml" are reserved future modes.
pub(crate) fn rekordbox_export(context: &SocketContext, mode: &str) -> Result<(), Error> {
    if mode != "comment" {
        return Err(anyhow::anyhow!("Unsupported export mode: {mode}"));
    }
    let sender = context.sender.clone();
    let db = context.db.clone();
    tokio::task::spawn_blocking(move || {
        let result = onetagger_db::export::export_comments(&db, &crate::socket::is_currently_playing, |done, total| {
            sender.send(json!({ "action": "rekordboxExportProgress", "done": done, "total": total })).ok();
        });
        match result {
            Ok(result) => {
                sender.send(json!({ "action": "rekordboxExportDone", "result": result })).ok();
            }
            Err(e) => {
                error!("Rekordbox export failed: {e}");
                sender.send(json!({ "action": "error", "message": format!("Export failed: {e}") })).ok();
            }
        }
    });
    Ok(())
}

/// Number of waveform bars sent to the UI
const WAVEFORM_BARS: usize = 180;
/// Stored bar scale: f32 (0..=1) quantized to a byte
const WAVEFORM_SCALE: f32 = 255.0;

fn waveform_message(track_id: i64, bars: &[u8]) -> Value {
    let bars: Vec<f32> = bars.iter().map(|b| *b as f32 / WAVEFORM_SCALE).collect();
    json!({ "action": "trackWaveform", "trackId": track_id, "bars": bars })
}

/// The most recently requested waveform; older in-flight generations cancel
/// themselves so rapid track-skipping doesn't pile up whole-file decodes
static LATEST_WAVEFORM_REQUEST: AtomicI64 = AtomicI64::new(0);
/// Unix seconds of the last on-demand waveform request (0 = never)
static LAST_WAVEFORM_REQUEST_AT: AtomicI64 = AtomicI64::new(0);

/// Unix seconds of the last user-driven waveform request; the background
/// precompute worker backs off while this is recent
pub(crate) fn last_waveform_request_at() -> i64 {
    LAST_WAVEFORM_REQUEST_AT.load(Ordering::SeqCst)
}

/// Decode a track's waveform and cache it (unless its duration is unknown).
/// With `interruptible`, a newer on-demand request cancels the decode and
/// `Ok(None)` is returned.
pub(crate) fn generate_and_cache(db: &Db, track_id: i64, interruptible: bool) -> Result<Option<Vec<u8>>, Error> {
    let path = tracks::get_track_path(db, track_id)?;
    let source = onetagger_player::AudioSources::from_path(&path)?;
    let duration = source.duration();
    let (rx, cancel) = source.generate_waveform(WAVEFORM_BARS as i16)?;
    let mut bars: Vec<u8> = Vec::with_capacity(WAVEFORM_BARS);
    for wave in rx {
        // A newer request supersedes this one: stop decoding
        if interruptible && LATEST_WAVEFORM_REQUEST.load(Ordering::SeqCst) != track_id {
            cancel.send(true).ok();
            return Ok(None);
        }
        bars.push((wave.clamp(0.0, 1.0) * WAVEFORM_SCALE) as u8);
        // Unknown duration degrades to one bar per sample buffer: don't
        // collect (or cache) garbage beyond the UI's bar count
        if bars.len() >= WAVEFORM_BARS {
            cancel.send(true).ok();
            break;
        }
    }
    bars.resize(WAVEFORM_BARS, 0);
    // A broken duration produces a meaningless waveform; show it once
    // but never cache it
    if duration > 0 {
        tracks::set_waveform(db, track_id, &bars)?;
    }
    Ok(Some(bars))
}

/// Waveform by track id: served from the DB cache, generated (and cached) otherwise
pub(crate) fn waveform_get(context: &SocketContext, track_id: i64) -> Result<(), Error> {
    if let Some(bars) = tracks::get_waveform(&context.db, track_id)? {
        return context.send(waveform_message(track_id, &bars));
    }
    LATEST_WAVEFORM_REQUEST.store(track_id, Ordering::SeqCst);
    LAST_WAVEFORM_REQUEST_AT.store(unix_now(), Ordering::SeqCst);
    let sender = context.sender.clone();
    let db = context.db.clone();
    tokio::task::spawn_blocking(move || {
        let result: Result<(), Error> = (|| {
            if let Some(bars) = generate_and_cache(&db, track_id, true)? {
                sender.send(waveform_message(track_id, &bars))?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            warn!("Waveform generation failed for track {track_id}: {e}");
        }
    });
    Ok(())
}

/// Current unix time in seconds
pub(crate) fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub(crate) fn flush_status_get(context: &SocketContext) -> Result<(), Error> {
    let (pending, errors) = flush::flush_status(&context.db)?;
    let errors: Vec<Value> = errors.into_iter()
        .map(|(track_id, error)| json!({ "trackId": track_id, "error": error }))
        .collect();
    context.send(json!({ "action": "flushStatus", "pending": pending, "errors": errors }))
}

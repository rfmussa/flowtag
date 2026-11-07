//! OS media integration glue.
//!
//! The GUI shell (crates/onetagger) owns the actual souvlaki controls (macOS
//! Now Playing / MPRIS) because they must live on the main thread with the
//! event loop. This module is the bridge: the websocket layer pushes the
//! client's player state in through a registered sink, and media-key commands
//! from the OS ride the existing broadcast channel out to every client.
//! In server mode no sink is registered and everything is a no-op.

use std::sync::OnceLock;
use serde::{Deserialize, Serialize};

/// Client player state, as reported over the websocket
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NowPlaying {
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub path: String,
}

static MEDIA_SINK: OnceLock<Box<dyn Fn(NowPlaying) + Send + Sync>> = OnceLock::new();
static MEDIA_BROADCAST: OnceLock<tokio::sync::broadcast::Sender<String>> = OnceLock::new();

/// Register the shell's Now Playing sink. Call before `start_all`; only the
/// first registration wins.
pub fn register_media_sink(sink: impl Fn(NowPlaying) + Send + Sync + 'static) {
    MEDIA_SINK.set(Box::new(sink)).ok();
}

/// Wired to the per-connection broadcast channel at server start
pub(crate) fn set_media_broadcast(tx: tokio::sync::broadcast::Sender<String>) {
    MEDIA_BROADCAST.set(tx).ok();
}

/// Forward the client's player state to the OS (no-op in server mode)
pub(crate) fn push_now_playing(now_playing: NowPlaying) {
    if let Some(sink) = MEDIA_SINK.get() {
        sink(now_playing);
    }
}

/// Forward an OS media command to every connected client. Safe from any thread.
pub fn media_key(key: &str, position_ms: Option<u64>) {
    if let Some(tx) = MEDIA_BROADCAST.get() {
        let message = serde_json::json!({
            "action": "mediaKey",
            "key": key,
            "positionMs": position_ms,
        });
        tx.send(message.to_string()).ok();
    }
}

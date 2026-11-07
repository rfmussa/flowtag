use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A library root folder
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbFolder {
    pub id: i64,
    pub path: String,
}

/// A track row as sent to the client (tag ids resolved)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbTrack {
    pub id: i64,
    pub path: String,
    pub format: String,
    pub title: String,
    pub artist: String,
    pub duration_ms: Option<i64>,
    pub bpm: Option<i64>,
    pub key: Option<String>,
    pub year: Option<i64>,
    pub missing: bool,
    pub needs_flush: bool,
    pub tag_ids: Vec<i64>,
}

/// A tag in the pool
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbTag {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub group_id: Option<i64>,
    /// Client-defined keybind blob (opaque JSON)
    pub keybind: Option<Value>,
    /// How many tracks currently carry this tag
    pub count: i64,
}

/// A tag group
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbGroup {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub position: i64,
}

/// Data needed to insert/update a track row during scan
#[derive(Debug, Clone)]
pub struct ScannedTrack {
    pub path: String,
    pub format: String,
    pub title: String,
    pub artist: String,
    pub duration_ms: Option<i64>,
    pub bpm: Option<i64>,
    pub key: Option<String>,
    pub year: Option<i64>,
    pub mtime: i64,
    pub size: i64,
    /// Tag names read from the file's TAGS field
    pub tag_names: Vec<String>,
}

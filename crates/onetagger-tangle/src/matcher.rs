//! Matching FlowTag tracks to Tangle items: bulk from the collection export,
//! then one-by-one lookups for whatever is still unmatched.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use rusqlite::{params, Connection};
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use crate::client::Client;
use crate::error::TangleError;
use crate::refs::local_ref;

/// How long a track that was not found in Tangle is left alone before asking again.
const RECHECK_AFTER_S: i64 = 6 * 3600;
/// Most unmatched paths kept in a `MatchSummary`.
const MAX_UNMATCHED_PATHS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchSummary {
    pub matched: usize,
    pub total: usize,
    pub unmatched_paths: Vec<String>,
}

fn nfc(s: &str) -> String {
    s.nfc().collect()
}

/// Copy refs of local sources: source id -> (ref -> item id).
fn local_copies(records: &[Value]) -> BTreeMap<String, HashMap<String, String>> {
    let local: BTreeSet<&str> = records
        .iter()
        .filter(|r| r["type"] == "source" && r["platform"] == "local")
        .filter_map(|r| r["id"].as_str())
        .collect();
    let mut out: BTreeMap<String, HashMap<String, String>> =
        local.iter().map(|id| (id.to_string(), HashMap::new())).collect();
    for item in records.iter().filter(|r| r["type"] == "item") {
        let Some(item_id) = item["id"].as_str() else { continue };
        for copy in item["copies"].as_array().into_iter().flatten() {
            let (Some(src), Some(r)) = (copy["source"].as_str(), copy["ref"].as_str()) else { continue };
            if let Some(refs) = out.get_mut(src) {
                refs.entry(nfc(r)).or_insert_with(|| item_id.to_string());
            }
        }
    }
    out
}

/// The local source (smallest id if several) with a copy under the folder's `~/…/` prefix.
fn source_for_folder<'a>(
    sources: &'a BTreeMap<String, HashMap<String, String>>,
    folder_path: &str,
    home: &str,
) -> Option<&'a str> {
    let prefix = format!("{}/", local_ref(folder_path, home)?);
    sources
        .iter()
        .find(|(_, refs)| refs.keys().any(|r| r.starts_with(&prefix)))
        .map(|(id, _)| id.as_str())
}

/// Fill `tangle_sources` and `tangle_items` from a Tangle collection export.
/// Every present track gets a `tangle_items` row (`item_id` NULL when not found).
pub fn match_from_collection(
    c: &Connection,
    records: &[Value],
    home: &str,
    now: i64,
) -> rusqlite::Result<MatchSummary> {
    let sources = local_copies(records);
    let tx = c.unchecked_transaction()?;
    let folders: Vec<(i64, String)> = tx
        .prepare("SELECT id, path FROM folders ORDER BY id")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut summary = MatchSummary { matched: 0, total: 0, unmatched_paths: Vec::new() };
    for (folder_id, folder_path) in folders {
        let source = source_for_folder(&sources, &folder_path, home);
        match source {
            Some(src) => {
                tx.execute(
                    "INSERT OR REPLACE INTO tangle_sources (folder_id, source_id) VALUES (?1, ?2)",
                    params![folder_id, src],
                )?;
            }
            None => {
                tx.execute("DELETE FROM tangle_sources WHERE folder_id = ?1", [folder_id])?;
            }
        }
        let tracks: Vec<(i64, String)> = tx
            .prepare("SELECT id, path FROM tracks WHERE folder_id = ?1 AND missing = 0 ORDER BY id")?
            .query_map([folder_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (track_id, path) in tracks {
            let item = source
                .and_then(|src| sources.get(src))
                .zip(local_ref(&path, home))
                .and_then(|(refs, r)| refs.get(&r));
            tx.execute(
                "INSERT OR REPLACE INTO tangle_items (track_id, item_id, checked_at) VALUES (?1, ?2, ?3)",
                params![track_id, item, now],
            )?;
            summary.total += 1;
            if item.is_some() {
                summary.matched += 1;
            } else if summary.unmatched_paths.len() < MAX_UNMATCHED_PATHS {
                summary.unmatched_paths.push(path);
            }
        }
    }
    tx.commit()?;
    Ok(summary)
}

pub(crate) fn db_error(e: rusqlite::Error) -> TangleError {
    log::error!("tangle matcher database error: {e}");
    TangleError::Other { status: 0, code: "db_error".into(), request_id: None }
}

/// Tangle refused this one lookup for good (a 4xx that is not auth, backup or rate
/// limiting): retrying it every pass would stall all sync at the same track.
fn is_rejected_lookup(e: &TangleError) -> bool {
    match e {
        TangleError::ItemNotFound => true,
        TangleError::Other { status, .. } => (400..500).contains(status),
        _ => false,
    }
}

/// Ask Tangle about tracks that have no `tangle_items` row yet, or were not found
/// more than six hours ago. At most `limit` tracks are handled; returns how many.
/// Tracks in folders without a Tangle source are marked checked without a request.
/// A lookup Tangle rejects with a 4xx marks that track checked (not found) and moves on;
/// any other `TangleError` stops the run immediately (rows written so far are kept).
pub fn lookup_unmatched(
    c: &Connection,
    client: &Client,
    home: &str,
    now: i64,
    limit: usize,
    pace: Duration,
) -> Result<usize, TangleError> {
    let pending: Vec<(i64, String, Option<String>)> = c
        .prepare(
            "SELECT t.id, t.path, ts.source_id
             FROM tracks t
             LEFT JOIN tangle_items ti ON ti.track_id = t.id
             LEFT JOIN tangle_sources ts ON ts.folder_id = t.folder_id
             WHERE t.missing = 0
               AND (ti.track_id IS NULL OR (ti.item_id IS NULL AND ti.checked_at < ?1))
             ORDER BY t.id LIMIT ?2",
        )
        .and_then(|mut s| {
            s.query_map(params![now - RECHECK_AFTER_S, limit as i64], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?
            .collect()
        })
        .map_err(db_error)?;

    let mut handled = 0;
    let mut requested = false;
    for (track_id, path, source) in pending {
        let item = match (source, local_ref(&path, home)) {
            (Some(src), Some(r)) => {
                if requested {
                    std::thread::sleep(pace);
                }
                requested = true;
                match client.find_item(&src, &r) {
                    Ok(item) => item,
                    Err(e) if is_rejected_lookup(&e) => {
                        log::warn!("Tangle lookup of track {track_id} rejected, skipping it: {e}");
                        None
                    }
                    Err(e) => return Err(e),
                }
            }
            _ => None,
        };
        c.execute(
            "INSERT OR REPLACE INTO tangle_items (track_id, item_id, checked_at) VALUES (?1, ?2, ?3)",
            params![track_id, item, now],
        )
        .map_err(db_error)?;
        handled += 1;
    }
    Ok(handled)
}

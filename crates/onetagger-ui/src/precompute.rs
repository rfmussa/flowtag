//! Low-priority background worker that pre-generates missing waveforms so the
//! first playback during a sorting session is instant.

use std::collections::HashSet;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use onetagger_db::{tracks, Db};

use crate::tagflow::{generate_and_cache, last_waveform_request_at, unix_now};

/// Pause after each precomputed track
const PACE: Duration = Duration::from_millis(250);
/// Pause while the user is actively requesting waveforms
const BACKOFF: Duration = Duration::from_secs(2);
/// Pause when there is nothing left to do
const IDLE: Duration = Duration::from_secs(30);
/// A request this recent (seconds) counts as "user is active"
const ACTIVE_WINDOW_SECS: i64 = 3;
/// Log progress every N generated waveforms
const LOG_EVERY: usize = 100;

/// Start the precompute worker thread (runs for the process lifetime)
pub fn spawn_waveform_precompute(db: Arc<Db>) {
    let spawned = thread::Builder::new()
        .name("waveform-precompute".to_string())
        .spawn(move || run(&db));
    if let Err(e) = spawned {
        error!("Failed starting waveform precompute thread: {e}");
    }
}

/// Next pending track id that has not already failed this process lifetime
fn next_pending(db: &Db, failed: &HashSet<i64>) -> Result<Option<i64>, anyhow::Error> {
    let limit = failed.len() + 1;
    let ids = tracks::tracks_without_waveform(db, limit)?;
    Ok(ids.into_iter().find(|id| !failed.contains(id)))
}

fn run(db: &Db) {
    let mut failed: HashSet<i64> = HashSet::new();
    let mut done = 0usize;
    let mut working = false;
    loop {
        if unix_now() - last_waveform_request_at() < ACTIVE_WINDOW_SECS {
            thread::sleep(BACKOFF);
            continue;
        }
        // Never read files while the comment export is rewriting them: a decode
        // of a half-written file fails and would blacklist the track
        if onetagger_db::export::EXPORT_ACTIVE.load(std::sync::atomic::Ordering::SeqCst) {
            thread::sleep(BACKOFF);
            continue;
        }
        let id = match next_pending(db, &failed) {
            Ok(Some(id)) => id,
            Ok(None) => {
                if working {
                    info!("Waveform precompute finished backlog ({done} generated, {} failed)", failed.len());
                    working = false;
                }
                thread::sleep(IDLE);
                continue;
            }
            Err(e) => {
                debug!("Waveform precompute query failed: {e}");
                thread::sleep(IDLE);
                continue;
            }
        };
        working = true;
        match generate_and_cache(db, id, false) {
            Ok(Some(_)) => {
                done += 1;
                if done % LOG_EVERY == 0 {
                    info!("Waveform precompute: {done} generated");
                }
            }
            Ok(None) => {}
            Err(e) => {
                debug!("Waveform precompute failed for track {id}: {e}");
                failed.insert(id);
            }
        }
        thread::sleep(PACE);
    }
}

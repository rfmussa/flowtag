//! Audition analysis runner (experimental): walks tracks without an analysis
//! row, measures integrated loudness (ebur128) and derives section markers
//! from the cached waveform bars (generating them when missing, which also
//! warms the waveform cache). Runs on a blocking task; progress streams to
//! the requesting window.

use anyhow::Error;
use onetagger_db::{analysis, tracks, Db};
use onetagger_player::AudioSources;
use onetagger_player::rodio::Source;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::sender::MessageSender;

/// Only one analysis run at a time
static ANALYSIS_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Integrated LUFS of a file, decoded start to finish
fn measure_lufs(path: &str) -> Option<f64> {
    let source = AudioSources::from_path(path).ok()?;
    let samples = source.get_source().ok()?;
    let channels = samples.channels() as u32;
    let rate = samples.sample_rate();
    if channels == 0 || rate == 0 {
        return None;
    }
    let mut meter = ebur128::EbuR128::new(channels, rate, ebur128::Mode::I).ok()?;
    // Feed in ~1s interleaved chunks
    let chunk = (channels as usize) * (rate as usize);
    let mut buf: Vec<i16> = Vec::with_capacity(chunk);
    for s in samples {
        buf.push(s);
        if buf.len() >= chunk {
            meter.add_frames_i16(&buf).ok()?;
            buf.clear();
        }
    }
    // Drop a ragged tail shorter than one frame; feed the rest
    let whole = (buf.len() / channels as usize) * channels as usize;
    if whole > 0 {
        meter.add_frames_i16(&buf[..whole]).ok()?;
    }
    meter.loudness_global().ok().filter(|l| l.is_finite())
}

fn analyze_one(db: &Db, track_id: i64) -> Result<(), Error> {
    let path = tracks::get_track_path(db, track_id)?;
    let duration: Option<i64> = db.conn().query_row(
        "SELECT duration_ms FROM tracks WHERE id = ?1",
        [track_id], |r| r.get(0),
    )?;
    // Sections from cached bars (generate + cache when missing)
    let bars = match tracks::get_waveform(db, track_id)? {
        Some(bars) => Some(bars),
        None => crate::tagflow::generate_and_cache(db, track_id, false)?,
    };
    let (intro_ms, drop_ms, outro_ms) = match (&bars, duration) {
        (Some(bars), Some(d)) if d > 0 => analysis::detect_sections(bars, d),
        _ => (None, None, None),
    };
    let lufs = measure_lufs(&path);
    analysis::set(db, &analysis::TrackAnalysis { track_id, lufs, intro_ms, drop_ms, outro_ms })?;
    Ok(())
}

/// Run analysis over every unanalyzed present track (one run at a time)
pub(crate) fn analysis_run(sender: MessageSender, db: Arc<Db>, limit: Option<usize>) {
    if ANALYSIS_ACTIVE.swap(true, Ordering::SeqCst) {
        sender.send(json!({ "action": "error", "message": "Analysis is already running" })).ok();
        return;
    }
    tokio::task::spawn_blocking(move || {
        let result: Result<(), Error> = (|| {
            let ids = analysis::without_analysis(&db, limit.unwrap_or(usize::MAX))?;
            let total = ids.len();
            let mut failed = 0usize;
            info!("Audition analysis: {total} track(s) to analyze");
            for (done, id) in ids.iter().enumerate() {
                // Never read files the comment export is rewriting
                while onetagger_db::export::EXPORT_ACTIVE.load(Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
                if let Err(e) = analyze_one(&db, *id) {
                    warn!("Analysis failed for track {id}: {e}");
                    failed += 1;
                    // Park a failing track so the run can't loop on it
                    analysis::set(&db, &analysis::TrackAnalysis {
                        track_id: *id, lufs: None, intro_ms: None, drop_ms: None, outro_ms: None,
                    }).ok();
                }
                if done % 5 == 0 || done + 1 == total {
                    sender.send(json!({ "action": "analysisProgress", "done": done + 1, "total": total })).ok();
                }
            }
            info!("Audition analysis finished: {} analyzed, {failed} failed", total - failed);
            sender.send(json!({ "action": "analysisDone", "analyzed": total - failed, "failed": failed })).ok();
            Ok(())
        })();
        ANALYSIS_ACTIVE.store(false, Ordering::SeqCst);
        if let Err(e) = result {
            error!("Audition analysis aborted: {e}");
            sender.send(json!({ "action": "error", "message": format!("Analysis failed: {e}") })).ok();
        }
    });
}

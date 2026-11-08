//! Audition analysis (experimental): loudness and section markers per track.
//! Sections (intro / drop / outro) are derived from the cached waveform bars —
//! an amplitude envelope we already compute — so only loudness needs a decode.

use anyhow::Error;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::Db;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackAnalysis {
    pub track_id: i64,
    /// Integrated loudness (LUFS); None when measurement failed
    pub lufs: Option<f64>,
    pub intro_ms: Option<i64>,
    pub drop_ms: Option<i64>,
    pub outro_ms: Option<i64>,
}

/// Derive section markers from waveform bars (0-255 amplitude envelope).
/// Returns (intro_ms, drop_ms, outro_ms):
/// - intro ends where the smoothed envelope first reaches 25% of its peak
/// - the drop is the strongest sustained rise after the intro, within the
///   first 60% of the track (None when nothing rises convincingly)
/// - the outro starts where the envelope last falls below 25% of peak
pub fn detect_sections(bars: &[u8], duration_ms: i64) -> (Option<i64>, Option<i64>, Option<i64>) {
    let n = bars.len();
    if n < 8 || duration_ms <= 0 {
        return (None, None, None);
    }
    // Moving average, window 5
    let smoothed: Vec<f32> = (0..n)
        .map(|i| {
            let a = i.saturating_sub(2);
            let b = (i + 3).min(n);
            bars[a..b].iter().map(|v| *v as f32).sum::<f32>() / (b - a) as f32
        })
        .collect();
    let peak = smoothed.iter().cloned().fold(0f32, f32::max);
    if peak <= 1.0 {
        return (None, None, None); // silence
    }
    let threshold = 0.25 * peak;
    let intro = smoothed.iter().position(|v| *v >= threshold).unwrap_or(0);
    let outro = n - 1 - smoothed.iter().rev().position(|v| *v >= threshold).unwrap_or(0);

    // Strongest sustained rise: compare the mean of w bars before vs after
    let limit = ((n as f32) * 0.6) as usize;
    let w = (n / 16).clamp(2, 8);
    let mut best_rise = 0f32;
    let mut best_at = intro;
    let from = intro.max(w);
    let to = limit.min(n.saturating_sub(w));
    let mut i = from;
    while i < to {
        let before: f32 = smoothed[i - w..i].iter().sum::<f32>() / w as f32;
        let after: f32 = smoothed[i..i + w].iter().sum::<f32>() / w as f32;
        let rise = after - before;
        if rise > best_rise {
            best_rise = rise;
            best_at = i;
        }
        i += 1;
    }
    let drop = (best_rise >= 0.12 * peak).then_some(best_at);

    let to_ms = |idx: usize| duration_ms * idx as i64 / n as i64;
    (Some(to_ms(intro)), drop.map(to_ms), Some(to_ms(outro)))
}

pub fn set(db: &Db, a: &TrackAnalysis) -> Result<(), Error> {
    db.conn().execute(
        "INSERT OR REPLACE INTO analysis (track_id, lufs, intro_ms, drop_ms, outro_ms)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![a.track_id, a.lufs, a.intro_ms, a.drop_ms, a.outro_ms],
    )?;
    Ok(())
}

fn row_to_analysis(r: &rusqlite::Row) -> rusqlite::Result<TrackAnalysis> {
    Ok(TrackAnalysis {
        track_id: r.get(0)?,
        lufs: r.get(1)?,
        intro_ms: r.get(2)?,
        drop_ms: r.get(3)?,
        outro_ms: r.get(4)?,
    })
}

pub fn get(db: &Db, track_id: i64) -> Result<Option<TrackAnalysis>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT track_id, lufs, intro_ms, drop_ms, outro_ms FROM analysis WHERE track_id = ?1",
    )?;
    let mut rows = stmt.query_map([track_id], row_to_analysis)?;
    Ok(rows.next().transpose()?)
}

pub fn lufs_for_path(db: &Db, path: &str) -> Result<Option<f64>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT a.lufs FROM analysis a JOIN tracks t ON t.id = a.track_id WHERE t.path = ?1",
    )?;
    let mut rows = stmt.query_map([path], |r| r.get::<_, Option<f64>>(0))?;
    Ok(rows.next().transpose()?.flatten())
}

/// Present tracks that still need analysis, oldest first
pub fn without_analysis(db: &Db, limit: usize) -> Result<Vec<i64>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT t.id FROM tracks t
         LEFT JOIN analysis a ON a.track_id = t.id
         WHERE t.missing = 0 AND a.track_id IS NULL
         ORDER BY t.id LIMIT ?1",
    )?;
    let ids = stmt.query_map([limit as i64], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

/// (analyzed, present-total)
pub fn counts(db: &Db) -> Result<(i64, i64), Error> {
    let conn = db.conn();
    let analyzed: i64 = conn.query_row(
        "SELECT COUNT(*) FROM analysis a JOIN tracks t ON t.id = a.track_id AND t.missing = 0",
        [], |r| r.get(0),
    )?;
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM tracks WHERE missing = 0", [], |r| r.get(0))?;
    Ok((analyzed, total))
}

#[cfg(test)]
mod tests {
    use super::detect_sections;

    /// Synthetic envelope: silence, quiet intro, loud body, silent tail
    fn synthetic() -> Vec<u8> {
        let mut bars = vec![0u8; 20];
        bars.extend(vec![60u8; 40]);   // intro groove
        bars.extend(vec![200u8; 110]); // body (drop at idx 60)
        bars.extend(vec![0u8; 10]);    // tail
        bars
    }

    #[test]
    fn detects_intro_drop_and_outro() {
        let bars = synthetic();
        let duration = 360_000i64; // 6 minutes, 180 bars -> 2s per bar
        let (intro, drop, outro) = detect_sections(&bars, duration);
        let intro = intro.unwrap();
        let drop = drop.unwrap();
        let outro = outro.unwrap();
        // Intro ends where the quiet groove starts (~idx 20 = 40s), smoothing slack allowed
        assert!((30_000..=50_000).contains(&intro), "intro at {intro}");
        // Drop at the quiet->loud step (~idx 60 = 120s)
        assert!((105_000..=135_000).contains(&drop), "drop at {drop}");
        // Outro where the tail goes silent (~idx 169 = 338s)
        assert!((325_000..=352_000).contains(&outro), "outro at {outro}");
        assert!(intro < drop && drop < outro);
    }

    #[test]
    fn flat_track_has_no_drop() {
        let bars = vec![150u8; 180];
        let (intro, drop, outro) = detect_sections(&bars, 300_000);
        assert_eq!(drop, None, "constant energy is not a drop");
        assert_eq!(intro, Some(0));
        assert!(outro.unwrap() >= 295_000);
    }

    #[test]
    fn silence_yields_nothing() {
        assert_eq!(detect_sections(&vec![0u8; 180], 300_000), (None, None, None));
        assert_eq!(detect_sections(&[], 300_000), (None, None, None));
    }

    #[test]
    fn analysis_rows_roundtrip_and_cascade() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::Db::open(dir.path().join("t.db")).unwrap();
        {
            let conn = db.conn();
            conn.execute("INSERT INTO folders (path) VALUES ('/x')", []).unwrap();
            conn.execute(
                "INSERT INTO tracks (folder_id, path, format, title, mtime, size) VALUES (1, '/x/a.mp3', 'mp3', 'A', 1, 1)",
                [],
            ).unwrap();
        }
        assert_eq!(super::without_analysis(&db, 10).unwrap(), vec![1]);
        super::set(&db, &super::TrackAnalysis {
            track_id: 1, lufs: Some(-9.4), intro_ms: Some(30_000), drop_ms: Some(120_000), outro_ms: Some(330_000),
        }).unwrap();
        assert!(super::without_analysis(&db, 10).unwrap().is_empty());
        let a = super::get(&db, 1).unwrap().unwrap();
        assert_eq!(a.lufs, Some(-9.4));
        assert_eq!(super::lufs_for_path(&db, "/x/a.mp3").unwrap(), Some(-9.4));
        assert_eq!(super::counts(&db).unwrap(), (1, 1));
        db.conn().execute("DELETE FROM tracks WHERE id = 1", []).unwrap();
        assert!(super::get(&db, 1).unwrap().is_none(), "analysis cascades with the track");
    }
}

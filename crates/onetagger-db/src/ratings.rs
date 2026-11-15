//! Star ratings: 1–5 per track, NULL = unrated. Setting one dirties the track
//! for the flusher (same needs_flush / tag_version pipeline as tag edits) and
//! marks `rating_pending`, so the flusher writes the rating into the file via
//! `onetagger-tag`'s `set_rating` (POPM for MP3/AIFF).

use anyhow::{bail, Error};
use rusqlite::params;

use crate::{now, Db};

/// Lowest and highest star rating
pub const MIN_RATING: i64 = 1;
pub const MAX_RATING: i64 = 5;

/// Most tracks one rating request may touch (a whole-library multi-select fits)
pub const MAX_RATING_BATCH: usize = 100_000;

/// Validate a client-supplied rating: None clears, Some(1..=5) sets, anything else is rejected
pub fn validate_rating(rating: Option<i64>) -> Result<Option<u8>, Error> {
    match rating {
        None => Ok(None),
        Some(r) if (MIN_RATING..=MAX_RATING).contains(&r) => Ok(Some(r as u8)),
        Some(r) => bail!("Rating must be {MIN_RATING}-{MAX_RATING} stars or empty, got {r}"),
    }
}

/// A rating as read from a file, kept only when it is a real 1–5 value
pub fn rating_from_file(rating: Option<u8>) -> Option<i64> {
    rating.map(i64::from).filter(|r| (MIN_RATING..=MAX_RATING).contains(r))
}

/// Set (or with None clear) the rating of many tracks in one transaction.
/// Tracks that already carry exactly this rating are untouched (not dirtied).
/// Returns the ids that actually changed.
pub fn set_rating(db: &Db, track_ids: &[i64], rating: Option<i64>) -> Result<Vec<i64>, Error> {
    let rating = validate_rating(rating)?;
    if track_ids.is_empty() {
        bail!("No tracks to rate");
    }
    if track_ids.len() > MAX_RATING_BATCH {
        bail!("Too many tracks to rate at once ({})", track_ids.len());
    }
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let now = now();
    let mut changed = Vec::new();
    {
        let mut stmt = tx.prepare(
            "UPDATE tracks SET rating = ?2, rating_pending = 1, needs_flush = 1,
                               tag_updated_at = ?3, tag_version = tag_version + 1
             WHERE id = ?1 AND rating IS NOT ?2",
        )?;
        for &id in track_ids {
            if stmt.execute(params![id, rating, now])? > 0 {
                changed.push(id);
            }
        }
    }
    tx.commit()?;
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{insert_track, test_db};

    #[test]
    fn validation_accepts_one_to_five_and_none_only() {
        assert_eq!(validate_rating(None).unwrap(), None);
        for r in 1..=5 {
            assert_eq!(validate_rating(Some(r)).unwrap(), Some(r as u8));
        }
        for bad in [0, 6, -1, 255, 256, i64::MAX, i64::MIN] {
            assert!(validate_rating(Some(bad)).is_err(), "{bad} accepted");
        }
    }

    #[test]
    fn file_ratings_outside_range_are_dropped() {
        assert_eq!(rating_from_file(None), None);
        assert_eq!(rating_from_file(Some(0)), None);
        assert_eq!(rating_from_file(Some(3)), Some(3));
        assert_eq!(rating_from_file(Some(6)), None);
    }

    fn row(db: &Db, id: i64) -> (Option<i64>, i64, i64, i64) {
        db.conn().query_row(
            "SELECT rating, rating_pending, needs_flush, tag_version FROM tracks WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).unwrap()
    }

    #[test]
    fn set_marks_dirty_and_pending_and_skips_noops() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        let b = insert_track(&db, "/test-root/b.mp3");
        assert_eq!(set_rating(&db, &[a], Some(3)).unwrap(), vec![a]);
        assert_eq!(row(&db, a), (Some(3), 1, 1, 1));

        // b changes, a already has 3 and stays untouched
        assert_eq!(set_rating(&db, &[a, b], Some(3)).unwrap(), vec![b]);
        assert_eq!(row(&db, a).3, 1, "no-op must not bump tag_version");

        // Clear
        assert_eq!(set_rating(&db, &[a, b], None).unwrap(), vec![a, b]);
        assert_eq!(row(&db, a).0, None);
        assert!(set_rating(&db, &[a, b], None).unwrap().is_empty());
    }

    #[test]
    fn invalid_requests_change_nothing() {
        let (_dir, db) = test_db();
        let a = insert_track(&db, "/test-root/a.mp3");
        assert!(set_rating(&db, &[a], Some(0)).is_err());
        assert!(set_rating(&db, &[a], Some(6)).is_err());
        assert!(set_rating(&db, &[], Some(3)).is_err());
        assert_eq!(row(&db, a), (None, 0, 0, 0));
        // Unknown ids are ignored, not created
        assert!(set_rating(&db, &[9999], Some(2)).unwrap().is_empty());
    }
}

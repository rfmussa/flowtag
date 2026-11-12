use std::collections::HashMap;
use anyhow::{bail, Error};
use rusqlite::{params, Connection};

use crate::models::{DbFolder, DbTrack, ScannedTrack};
use crate::validate::validate_folder;
use crate::{now, Db};

/// List library root folders; a folder never renamed shows its path basename
pub fn list_folders(db: &Db) -> Result<Vec<DbFolder>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT id, path, name FROM folders ORDER BY path")?;
    let folders = stmt
        .query_map([], |r| {
            let path: String = r.get(1)?;
            let name: Option<String> = r.get(2)?;
            let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
                std::path::Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone())
            });
            Ok(DbFolder { id: r.get(0)?, path, name })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(folders)
}

/// Rename a source folder's display name (the path on disk is untouched)
pub fn rename_folder(db: &Db, id: i64, name: &str) -> Result<(), Error> {
    let name = name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(anyhow::anyhow!("Folder name must be 1-64 characters"));
    }
    db.conn().execute("UPDATE folders SET name = ?2 WHERE id = ?1", rusqlite::params![id, name])?;
    Ok(())
}

/// Add a library root folder (validated against existing roots, stored canonicalized)
pub fn add_folder(db: &Db, path: &str) -> Result<DbFolder, Error> {
    let existing = list_folders(db)?.into_iter().map(|f| f.path).collect::<Vec<_>>();
    let canonical = validate_folder(path, &existing)?;
    let conn = db.conn();
    conn.execute("INSERT INTO folders (path) VALUES (?1)", [&canonical])?;
    let name = std::path::Path::new(&canonical)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| canonical.clone());
    Ok(DbFolder { id: conn.last_insert_rowid(), path: canonical, name })
}

/// Remove a library root folder and (via cascade) all its tracks
pub fn remove_folder(db: &Db, id: i64) -> Result<(), Error> {
    let n = db.conn().execute("DELETE FROM folders WHERE id = ?1", [id])?;
    if n == 0 {
        bail!("Folder {id} does not exist");
    }
    Ok(())
}

/// All tracks with their tag ids, one query each way, merged in memory
pub fn list_tracks(db: &Db) -> Result<Vec<DbTrack>, Error> {
    let conn = db.conn();
    let mut tag_map: HashMap<i64, Vec<i64>> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT track_id, tag_id FROM track_tags ORDER BY track_id, position, tag_id")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows {
            let (track_id, tag_id) = row?;
            tag_map.entry(track_id).or_default().push(tag_id);
        }
    }
    let mut stmt = conn.prepare(
        "SELECT id, path, format, title, artist, duration_ms, missing, needs_flush, bpm, key, year, added_at, folder_id, done_at,
                EXISTS (SELECT 1 FROM staged s WHERE s.track_id = tracks.id),
                COALESCE((SELECT s.skip FROM staged s WHERE s.track_id = tracks.id), 0)
         FROM tracks ORDER BY path",
    )?;
    let tracks = stmt
        .query_map([], |r| {
            Ok(DbTrack {
                id: r.get(0)?,
                path: r.get(1)?,
                format: r.get(2)?,
                title: r.get(3)?,
                artist: r.get(4)?,
                duration_ms: r.get(5)?,
                missing: r.get::<_, i64>(6)? != 0,
                needs_flush: r.get::<_, i64>(7)? != 0,
                bpm: r.get(8)?,
                key: r.get(9)?,
                year: r.get(10)?,
                added_at: r.get(11)?,
                folder_id: r.get(12)?,
                done_at: r.get(13)?,
                staged: r.get::<_, i64>(14)? != 0,
                skip: r.get::<_, i64>(15)? != 0,
                tag_ids: vec![],
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tracks
        .into_iter()
        .map(|mut t| {
            t.tag_ids = tag_map.remove(&t.id).unwrap_or_default();
            t
        })
        .collect())
}

/// (path -> (mtime, size)) for every track under a folder, for scan diffing
pub fn track_stats_for_folder(db: &Db, folder_id: i64) -> Result<HashMap<String, (i64, i64)>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT path, mtime, size FROM tracks WHERE folder_id = ?1")?;
    let rows = stmt.query_map([folder_id], |r| Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))))?;
    let mut map = HashMap::new();
    for row in rows {
        let (path, stats) = row?;
        map.insert(path, stats);
    }
    Ok(map)
}

fn link_tag_names(conn: &Connection, track_id: i64, tag_names: &[String]) -> Result<(), Error> {
    for (pos, name) in tag_names.iter().enumerate() {
        // Skip malformed names coming from files rather than failing the scan
        let Ok((tag_id, _created)) = crate::tags::get_or_create_tag_conn(conn, name, None) else {
            continue;
        };
        conn.execute(
            "INSERT OR IGNORE INTO track_tags (track_id, tag_id, position) VALUES (?1, ?2, ?3)",
            params![track_id, tag_id, pos as i64],
        )?;
    }
    Ok(())
}

/// Upsert scanned tracks for a folder in one transaction.
/// New tracks get their file tags imported into the pool; changed tracks update
/// metadata (tags in the DB stay authoritative, so file tag values are only
/// imported when the track is new). Returns (added, updated).
pub fn upsert_tracks(db: &Db, folder_id: i64, tracks: &[ScannedTrack]) -> Result<(usize, usize), Error> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let mut added = 0;
    let mut updated = 0;
    for t in tracks {
        let existing: Option<i64> = tx
            .query_row("SELECT id FROM tracks WHERE path = ?1", [&t.path], |r| r.get(0))
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(e),
            })?;
        match existing {
            Some(id) => {
                tx.execute(
                    "UPDATE tracks SET format=?2, title=?3, artist=?4, duration_ms=?5, mtime=?6, size=?7, missing=0, folder_id=?8,
                                       bpm=?9, key=?10, year=?11
                     WHERE id = ?1",
                    params![id, t.format, t.title, t.artist, t.duration_ms, t.mtime, t.size, folder_id, t.bpm, t.key, t.year],
                )?;
                updated += 1;
            }
            None => {
                tx.execute(
                    "INSERT INTO tracks (folder_id, path, format, title, artist, duration_ms, mtime, size, bpm, key, year, added_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, unixepoch())",
                    params![folder_id, t.path, t.format, t.title, t.artist, t.duration_ms, t.mtime, t.size, t.bpm, t.key, t.year],
                )?;
                let id = tx.last_insert_rowid();
                link_tag_names(&tx, id, &t.tag_names)?;
                added += 1;
            }
        }
    }
    tx.commit()?;
    Ok((added, updated))
}

/// Sync the missing flag for a folder: tracks NOT in `seen_paths` become missing,
/// and previously-missing tracks that are seen again recover (their stats may be
/// unchanged, so the upsert diff alone would never clear the flag).
/// Returns how many went missing.
pub fn mark_missing(db: &Db, folder_id: i64, seen_paths: &[String]) -> Result<usize, Error> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    tx.execute("CREATE TEMP TABLE IF NOT EXISTS seen (path TEXT PRIMARY KEY)", [])?;
    tx.execute("DELETE FROM seen", [])?;
    {
        let mut stmt = tx.prepare("INSERT OR IGNORE INTO seen (path) VALUES (?1)")?;
        for p in seen_paths {
            stmt.execute([p])?;
        }
    }
    let n = tx.execute(
        "UPDATE tracks SET missing = 1 WHERE folder_id = ?1 AND missing = 0 AND path NOT IN (SELECT path FROM seen)",
        [folder_id],
    )?;
    let recovered = tx.execute(
        "UPDATE tracks SET missing = 0 WHERE folder_id = ?1 AND missing = 1 AND path IN (SELECT path FROM seen)",
        [folder_id],
    )?;
    if recovered > 0 {
        info!("{recovered} missing track(s) reappeared");
    }
    tx.execute("DELETE FROM seen", [])?;
    tx.commit()?;
    Ok(n)
}

/// Path of one track
pub fn get_track_path(db: &Db, track_id: i64) -> Result<String, Error> {
    Ok(db.conn().query_row("SELECT path FROM tracks WHERE id = ?1", [track_id], |r| r.get(0))?)
}

/// Cached waveform bars for a track (None when absent)
pub fn get_waveform(db: &Db, track_id: i64) -> Result<Option<Vec<u8>>, Error> {
    let conn = db.conn();
    let bars: Option<Vec<u8>> = conn
        .query_row("SELECT bars FROM waveforms WHERE track_id = ?1", [track_id], |r| r.get(0))
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e),
        })?;
    Ok(bars)
}

/// Store waveform bars for a track
pub fn set_waveform(db: &Db, track_id: i64, bars: &[u8]) -> Result<(), Error> {
    db.conn().execute(
        "INSERT OR REPLACE INTO waveforms (track_id, bars) VALUES (?1, ?2)",
        params![track_id, bars],
    )?;
    Ok(())
}

/// Ids of present (non-missing) tracks that have no cached waveform yet
pub fn tracks_without_waveform(db: &Db, limit: usize) -> Result<Vec<i64>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT t.id FROM tracks t WHERE t.missing = 0 \
         AND NOT EXISTS (SELECT 1 FROM waveforms w WHERE w.track_id = t.id) \
         ORDER BY t.id LIMIT ?1",
    )?;
    let ids = stmt
        .query_map([limit as i64], |r| r.get(0))?
        .collect::<Result<Vec<i64>, _>>()?;
    Ok(ids)
}

/// Update stored duration (filled lazily on first playback)
pub fn set_duration(db: &Db, track_id: i64, duration_ms: i64) -> Result<(), Error> {
    db.conn().execute(
        "UPDATE tracks SET duration_ms = ?2 WHERE id = ?1",
        params![track_id, duration_ms],
    )?;
    Ok(())
}

/// Mark one track as needing a flush (used by flushHint and tests)
pub fn mark_dirty(db: &Db, track_id: i64) -> Result<(), Error> {
    db.conn().execute(
        "UPDATE tracks SET needs_flush = 1, tag_updated_at = ?2, tag_version = tag_version + 1 WHERE id = ?1",
        params![track_id, now()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_db;

    fn scanned(path: &str, title: &str, tags: &[&str]) -> ScannedTrack {
        ScannedTrack {
            path: path.to_string(),
            format: "mp3".to_string(),
            title: title.to_string(),
            artist: "Artist".to_string(),
            duration_ms: Some(1000),
            bpm: Some(126),
            key: Some("8A".to_string()),
            year: Some(2020),
            mtime: 100,
            size: 200,
            tag_names: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn dj_metadata_roundtrips_through_upsert() {
        let (dir, db) = test_db();
        let root = dir.path().to_string_lossy().to_string();
        let f = add_folder(&db, &root).unwrap();
        upsert_tracks(&db, f.id, &[scanned("/a.mp3", "A", &[])]).unwrap();
        let t = &list_tracks(&db).unwrap()[0];
        assert_eq!(t.bpm, Some(126));
        assert_eq!(t.key.as_deref(), Some("8A"));
        assert_eq!(t.year, Some(2020));
    }

    #[test]
    fn folder_crud_and_validation() {
        let (dir, db) = test_db();
        let root = dir.path().to_string_lossy().to_string();
        let f = add_folder(&db, &root).unwrap();
        // Duplicate/nested rejected
        assert!(add_folder(&db, &root).is_err());
        remove_folder(&db, f.id).unwrap();
        assert!(list_folders(&db).unwrap().is_empty());
        assert!(remove_folder(&db, 42).is_err());
    }

    #[test]
    fn upsert_imports_tags_only_for_new_tracks() {
        let (dir, db) = test_db();
        let root = dir.path().to_string_lossy().to_string();
        let f = add_folder(&db, &root).unwrap();

        let (added, updated) = upsert_tracks(&db, f.id, &[scanned("/a.mp3", "A", &["dark", "rolling"])]).unwrap();
        assert_eq!((added, updated), (1, 0));
        let tracks = list_tracks(&db).unwrap();
        assert_eq!(tracks[0].tag_ids.len(), 2);

        // Re-scan with different file tags: DB stays authoritative
        let (added, updated) = upsert_tracks(&db, f.id, &[scanned("/a.mp3", "A2", &["other"])]).unwrap();
        assert_eq!((added, updated), (0, 1));
        let tracks = list_tracks(&db).unwrap();
        assert_eq!(tracks[0].title, "A2");
        assert_eq!(tracks[0].tag_ids.len(), 2);
        // Malformed file tag names are skipped, not fatal
        let (added, _) = upsert_tracks(&db, f.id, &[scanned("/b.mp3", "B", &["#bad", "ok"])]).unwrap();
        assert_eq!(added, 1);
        let tracks = list_tracks(&db).unwrap();
        let b = tracks.iter().find(|t| t.path == "/b.mp3").unwrap();
        assert_eq!(b.tag_ids.len(), 1);
    }

    #[test]
    fn file_tag_import_dedupes_case_insensitively() {
        let (dir, db) = test_db();
        let root = dir.path().to_string_lossy().to_string();
        let f = add_folder(&db, &root).unwrap();
        // "DARK" from one file and "dark" from another must become ONE pool tag
        // (link_tag_names relies on the schema's UNIQUE COLLATE NOCASE)
        upsert_tracks(&db, f.id, &[
            scanned("/a.mp3", "A", &["DARK"]),
            scanned("/b.mp3", "B", &["dark"]),
        ]).unwrap();
        let pool = crate::tags::list_tags(&db).unwrap();
        assert_eq!(pool.len(), 1);
        assert_eq!(pool[0].count, 2);
    }

    #[test]
    fn folder_cascade_deletes_tracks() {
        let (dir, db) = test_db();
        let root = dir.path().to_string_lossy().to_string();
        let f = add_folder(&db, &root).unwrap();
        upsert_tracks(&db, f.id, &[scanned("/a.mp3", "A", &[])]).unwrap();
        remove_folder(&db, f.id).unwrap();
        assert!(list_tracks(&db).unwrap().is_empty());
    }

    #[test]
    fn missing_marking() {
        let (dir, db) = test_db();
        let root = dir.path().to_string_lossy().to_string();
        let f = add_folder(&db, &root).unwrap();
        upsert_tracks(&db, f.id, &[scanned("/a.mp3", "A", &[]), scanned("/b.mp3", "B", &[])]).unwrap();
        let n = mark_missing(&db, f.id, &["/a.mp3".to_string()]).unwrap();
        assert_eq!(n, 1);
        let tracks = list_tracks(&db).unwrap();
        assert!(tracks.iter().find(|t| t.path == "/b.mp3").unwrap().missing);
        assert!(!tracks.iter().find(|t| t.path == "/a.mp3").unwrap().missing);
    }

    #[test]
    fn missing_track_recovers_when_seen_again() {
        let (dir, db) = test_db();
        let root = dir.path().to_string_lossy().to_string();
        let f = add_folder(&db, &root).unwrap();
        upsert_tracks(&db, f.id, &[scanned("/a.mp3", "A", &[])]).unwrap();
        // Vanishes on one scan...
        mark_missing(&db, f.id, &[]).unwrap();
        assert!(list_tracks(&db).unwrap()[0].missing);
        // ...and reappears with UNCHANGED mtime/size (e.g. drive replugged): the
        // upsert diff skips it, so mark_missing alone must clear the flag
        mark_missing(&db, f.id, &["/a.mp3".to_string()]).unwrap();
        assert!(!list_tracks(&db).unwrap()[0].missing, "reappeared track stayed missing");
    }

    #[test]
    fn waveform_cache_roundtrip() {
        let (dir, db) = test_db();
        let root = dir.path().to_string_lossy().to_string();
        let f = add_folder(&db, &root).unwrap();
        upsert_tracks(&db, f.id, &[scanned("/a.mp3", "A", &[])]).unwrap();
        let id = list_tracks(&db).unwrap()[0].id;
        assert!(get_waveform(&db, id).unwrap().is_none());
        set_waveform(&db, id, &[1, 2, 3]).unwrap();
        assert_eq!(get_waveform(&db, id).unwrap().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn tracks_without_waveform_skips_cached_and_missing() {
        let (dir, db) = test_db();
        let root = dir.path().to_string_lossy().to_string();
        let f = add_folder(&db, &root).unwrap();
        upsert_tracks(&db, f.id, &[
            scanned("/a.mp3", "A", &[]),
            scanned("/b.mp3", "B", &[]),
            scanned("/c.mp3", "C", &[]),
        ]).unwrap();
        let ids: Vec<i64> = list_tracks(&db).unwrap().iter().map(|t| t.id).collect();
        assert_eq!(tracks_without_waveform(&db, 10).unwrap().len(), 3);
        set_waveform(&db, ids[0], &[1]).unwrap();
        mark_missing(&db, f.id, &["/a.mp3".to_string()]).unwrap();
        // only /a.mp3 stays present, and it is cached: nothing pending
        assert!(tracks_without_waveform(&db, 10).unwrap().is_empty());
        upsert_tracks(&db, f.id, &[scanned("/d.mp3", "D", &[])]).unwrap();
        let pending = tracks_without_waveform(&db, 10).unwrap();
        assert_eq!(pending.len(), 1);
        assert!(!ids.contains(&pending[0]));
        assert_eq!(tracks_without_waveform(&db, 0).unwrap().len(), 0);
    }
}

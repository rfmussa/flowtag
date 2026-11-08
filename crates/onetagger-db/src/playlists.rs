//! Playlists: manual (explicit membership in playlist_tracks) and smart
//! (a saved filter JSON the client evaluates against the live library).
//! The DB stores the filter opaquely; `filter IS NULL` means manual.

use anyhow::Error;
use rusqlite::params;

use crate::models::DbPlaylist;
use crate::Db;

fn validate_name(name: &str) -> Result<&str, Error> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow::anyhow!("Playlist name can't be empty"));
    }
    if name.len() > 64 {
        return Err(anyhow::anyhow!("Playlist name too long (max 64 characters)"));
    }
    Ok(name)
}

/// All playlists in order, each with its manual-membership count
/// (smart playlists always count 0 here; the client evaluates their filter)
pub fn list(db: &Db) -> Result<Vec<DbPlaylist>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.position, p.filter,
                (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id)
         FROM playlists p ORDER BY p.position, p.id",
    )?;
    let playlists = stmt
        .query_map([], |r| {
            Ok(DbPlaylist {
                id: r.get(0)?,
                name: r.get(1)?,
                position: r.get(2)?,
                filter: r.get(3)?,
                count: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(playlists)
}

/// Create a playlist; `filter` (opaque JSON) makes it smart. Returns the id.
pub fn create(db: &Db, name: &str, filter: Option<&str>) -> Result<i64, Error> {
    let name = validate_name(name)?;
    let conn = db.conn();
    conn.execute(
        "INSERT INTO playlists (name, filter, position)
         VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM playlists))",
        params![name, filter],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn rename(db: &Db, id: i64, name: &str) -> Result<(), Error> {
    let name = validate_name(name)?;
    db.conn().execute("UPDATE playlists SET name = ?2 WHERE id = ?1", params![id, name])?;
    Ok(())
}

pub fn delete(db: &Db, id: i64) -> Result<(), Error> {
    db.conn().execute("DELETE FROM playlists WHERE id = ?1", [id])?;
    Ok(())
}

/// Add tracks (deduplicated) to a manual playlist, appended in order.
/// Returns how many were actually added.
pub fn add_tracks(db: &Db, id: i64, track_ids: &[i64]) -> Result<usize, Error> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let mut added = 0;
    for track_id in track_ids {
        added += tx.execute(
            "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position)
             VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM playlist_tracks WHERE playlist_id = ?1))",
            params![id, track_id],
        )?;
    }
    tx.commit()?;
    Ok(added)
}

pub fn remove_tracks(db: &Db, id: i64, track_ids: &[i64]) -> Result<(), Error> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    for track_id in track_ids {
        tx.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
            params![id, track_id],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Manual membership, in playlist order
pub fn track_ids(db: &Db, id: i64) -> Result<Vec<i64>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position, track_id",
    )?;
    let ids = stmt.query_map([id], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use crate::Db;

    fn db_with_tracks(n: i64) -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("test.db")).unwrap();
        {
            let conn = db.conn();
            conn.execute("INSERT INTO folders (path) VALUES ('/x')", []).unwrap();
            for i in 1..=n {
                conn.execute(
                    "INSERT INTO tracks (folder_id, path, format, title, mtime, size)
                     VALUES (1, ?1, 'mp3', ?2, 1, 1)",
                    rusqlite::params![format!("/x/{i}.mp3"), format!("T{i}")],
                ).unwrap();
            }
        }
        (dir, db)
    }

    #[test]
    fn playlist_crud_and_membership() {
        let (_dir, db) = db_with_tracks(3);
        let id = super::create(&db, "  Warmup ", None).unwrap();

        // Dedup: adding t1 twice counts once
        assert_eq!(super::add_tracks(&db, id, &[1, 2]).unwrap(), 2);
        assert_eq!(super::add_tracks(&db, id, &[1, 3]).unwrap(), 1);
        assert_eq!(super::track_ids(&db, id).unwrap(), vec![1, 2, 3]);

        let listed = super::list(&db).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Warmup", "name is trimmed");
        assert_eq!(listed[0].count, 3);
        assert!(listed[0].filter.is_none());

        super::remove_tracks(&db, id, &[2]).unwrap();
        assert_eq!(super::track_ids(&db, id).unwrap(), vec![1, 3]);

        // Smart playlist stores its filter opaquely
        let smart = super::create(&db, "Peak hour", Some(r#"{"tagIds":[4]}"#)).unwrap();
        let listed = super::list(&db).unwrap();
        assert_eq!(listed.len(), 2);
        let s = listed.iter().find(|p| p.id == smart).unwrap();
        assert_eq!(s.filter.as_deref(), Some(r#"{"tagIds":[4]}"#));

        // Names are unique (case-insensitive)
        assert!(super::create(&db, "warmup", None).is_err());
        assert!(super::create(&db, "   ", None).is_err());

        super::rename(&db, id, "Openers").unwrap();
        assert_eq!(super::list(&db).unwrap().iter().find(|p| p.id == id).unwrap().name, "Openers");

        super::delete(&db, id).unwrap();
        let orphans: i64 = db.conn()
            .query_row("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?1", [id], |r| r.get(0))
            .unwrap();
        assert_eq!(orphans, 0, "membership rows cascade with the playlist");
    }

    #[test]
    fn deleting_a_track_cascades_membership() {
        let (_dir, db) = db_with_tracks(2);
        let id = super::create(&db, "List", None).unwrap();
        super::add_tracks(&db, id, &[1, 2]).unwrap();
        db.conn().execute("DELETE FROM tracks WHERE id = 1", []).unwrap();
        assert_eq!(super::track_ids(&db, id).unwrap(), vec![2]);
    }
}

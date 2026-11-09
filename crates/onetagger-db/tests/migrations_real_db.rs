//! Upgrade safety against a real library: open a snapshot of an existing
//! database through the normal Db::open path and prove the migration runner
//! touches neither the schema version nor any user data.
//!
//! Ignored by default (needs a snapshot). Run with:
//!   sqlite3 <live onetagger.db> ".backup /tmp/snap.db"
//!   FLOWTAG_REAL_DB_SNAPSHOT=/tmp/snap.db cargo test -p onetagger-db --test migrations_real_db -- --ignored

use onetagger_db::Db;

/// Every user-data table, as (name, ordered content digest)
fn fingerprint(conn: &rusqlite::Connection) -> Vec<(String, String)> {
    let queries = [
        ("tracks", "SELECT group_concat(id || '|' || path || '|' || title || '|' || artist || '|' || coalesce(bpm,'') || '|' || coalesce(key,'') || '|' || coalesce(year,'') || '|' || added_at || '|' || folder_id, char(10)) FROM (SELECT * FROM tracks ORDER BY id)"),
        ("tags", "SELECT group_concat(id || '|' || name || '|' || color || '|' || coalesce(group_id,'') || '|' || coalesce(keybind,''), char(10)) FROM (SELECT * FROM tags ORDER BY id)"),
        ("track_tags", "SELECT group_concat(track_id || '|' || tag_id || '|' || position, char(10)) FROM (SELECT * FROM track_tags ORDER BY track_id, tag_id)"),
        ("groups", "SELECT group_concat(id || '|' || name || '|' || color, char(10)) FROM (SELECT * FROM groups ORDER BY id)"),
        ("folders", "SELECT group_concat(id || '|' || path || '|' || coalesce(name,''), char(10)) FROM (SELECT * FROM folders ORDER BY id)"),
        ("playlists", "SELECT group_concat(id || '|' || name || '|' || coalesce(filter,''), char(10)) FROM (SELECT * FROM playlists ORDER BY id)"),
        ("playlist_tracks", "SELECT group_concat(playlist_id || '|' || track_id, char(10)) FROM (SELECT * FROM playlist_tracks ORDER BY playlist_id, track_id)"),
        ("analysis", "SELECT group_concat(track_id || '|' || coalesce(lufs,'') || '|' || coalesce(drop_ms,''), char(10)) FROM (SELECT * FROM analysis ORDER BY track_id)"),
        ("waveforms", "SELECT COUNT(*) || '|' || coalesce(sum(length(bars)), 0) FROM waveforms"),
    ];
    queries
        .iter()
        .map(|(name, sql)| {
            let v: Option<String> = conn.query_row(sql, [], |r| r.get(0)).unwrap();
            (name.to_string(), v.unwrap_or_default())
        })
        .collect()
}

#[test]
#[ignore]
fn real_db_upgrade_preserves_everything() {
    let snapshot = match std::env::var("FLOWTAG_REAL_DB_SNAPSHOT") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("SKIP: set FLOWTAG_REAL_DB_SNAPSHOT to a database snapshot");
            return;
        }
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("onetagger.db");
    std::fs::copy(&snapshot, &path).unwrap();

    let (version_before, before) = {
        let conn = rusqlite::Connection::open(&path).unwrap();
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        (v, fingerprint(&conn))
    };

    // The normal app path: open runs the migration runner
    let db = Db::open(&path).unwrap();
    let conn = db.conn();
    let version_after: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    let after = fingerprint(&conn);
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();

    assert_eq!(version_after, version_before, "an up-to-date library must not be re-migrated");
    assert_eq!(integrity, "ok");
    for ((name, b), (_, a)) in before.iter().zip(after.iter()) {
        assert_eq!(b, a, "table {name} changed during open");
    }
    eprintln!("real DB at v{version_after}: {} tables unchanged", before.len());
}

/// A multi-step upgrade on real data: roll the snapshot back to v5 so v6-v8
/// replay as one transaction over the whole library, then prove nothing moved
#[test]
#[ignore]
fn real_db_multi_step_upgrade_preserves_everything() {
    let snapshot = match std::env::var("FLOWTAG_REAL_DB_SNAPSHOT") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("SKIP: set FLOWTAG_REAL_DB_SNAPSHOT to a database snapshot");
            return;
        }
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("onetagger.db");
    std::fs::copy(&snapshot, &path).unwrap();

    let before = {
        let conn = rusqlite::Connection::open(&path).unwrap();
        let fp = fingerprint(&conn);
        conn.pragma_update(None, "user_version", 5).unwrap();
        fp
    };

    let db = Db::open(&path).unwrap();
    let conn = db.conn();
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    let after = fingerprint(&conn);
    assert_eq!(version, 8, "v6-v8 must replay to the latest version");
    for ((name, b), (_, a)) in before.iter().zip(after.iter()) {
        assert_eq!(b, a, "table {name} changed during the v5 -> v8 upgrade");
    }
    eprintln!("real DB v5 -> v{version}: {} tables unchanged", before.len());
}

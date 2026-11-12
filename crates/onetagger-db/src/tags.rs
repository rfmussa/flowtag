use anyhow::{bail, Error};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::models::{DbGroup, DbTag};
use crate::staging::log_or_warn;
use crate::validate::{validate_color, validate_tag_name};
use crate::{now, Db};

/// Legacy default color (the old OneTagger teal). New tags no longer get it;
/// migration v4 recolors tags that still wear it.
pub const DEFAULT_TAG_COLOR: &str = "#00d2bf";

/// Curated palette for auto-assigned tag colors: 12 distinct hues tuned for a
/// dark UI (mid-lightness, readable as 7px dots and chip fills). New tags take
/// the least-used color so the pool stays visually spread.
pub const TAG_COLOR_PALETTE: &[&str] = &[
    "#e5484d", // red
    "#f76b15", // orange
    "#ffb224", // amber
    "#92c94e", // lime
    "#30a46c", // green
    "#0091ff", // blue
    "#3e63dd", // indigo
    "#6e56cf", // violet
    "#8e4ec6", // purple
    "#d6409f", // magenta
    "#e93d82", // pink
    "#ad7f58", // bronze
];

/// Pick the least-used palette color (ties broken by palette order), so
/// auto-assigned colors spread evenly across the pool.
fn next_tag_color(conn: &Connection) -> Result<&'static str, Error> {
    let mut stmt = conn.prepare("SELECT color, COUNT(*) FROM tags GROUP BY color")?;
    let counts: std::collections::HashMap<String, i64> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let color = TAG_COLOR_PALETTE
        .iter()
        .min_by_key(|c| counts.get(**c).copied().unwrap_or(0))
        .expect("palette is non-empty");
    Ok(color)
}

fn row_to_tag(row: &rusqlite::Row) -> rusqlite::Result<DbTag> {
    Ok(DbTag {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        group_id: row.get(3)?,
        keybind: row
            .get::<_, Option<String>>(4)?
            .and_then(|s| serde_json::from_str(&s).ok()),
        count: row.get(5)?,
    })
}

const TAG_SELECT: &str = "
    SELECT t.id, t.name, t.color, t.group_id, t.keybind,
           (SELECT COUNT(*) FROM track_tags tt WHERE tt.tag_id = t.id) AS count
    FROM tags t";

/// List the whole tag pool with usage counts
pub fn list_tags(db: &Db) -> Result<Vec<DbTag>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare(&format!("{TAG_SELECT} ORDER BY t.name COLLATE NOCASE"))?;
    let tags = stmt.query_map([], row_to_tag)?.collect::<Result<Vec<_>, _>>()?;
    Ok(tags)
}

fn get_tag_inner(conn: &Connection, id: i64) -> Result<DbTag, Error> {
    Ok(conn.query_row(&format!("{TAG_SELECT} WHERE t.id = ?1"), [id], row_to_tag)?)
}

/// Get one tag by id
pub fn get_tag(db: &Db, id: i64) -> Result<DbTag, Error> {
    get_tag_inner(&db.conn(), id)
}

fn find_tag_by_name(conn: &Connection, name: &str) -> Result<Option<i64>, Error> {
    Ok(conn
        .query_row("SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE", [name], |r| r.get(0))
        .optional()?)
}

/// Connection-level get-or-create, shared by the public API and the scanner's
/// file-tag import. Returns (tag_id, created).
pub(crate) fn get_or_create_tag_conn(conn: &Connection, name: &str, color: Option<&str>) -> Result<(i64, bool), Error> {
    let name = validate_tag_name(name)?;
    if let Some(id) = find_tag_by_name(conn, &name)? {
        return Ok((id, false));
    }
    let color = match color {
        Some(c) => c,
        None => next_tag_color(conn)?,
    };
    validate_color(color)?;
    conn.execute("INSERT INTO tags (name, color) VALUES (?1, ?2)", params![name, color])?;
    Ok((conn.last_insert_rowid(), true))
}

fn tag_name(conn: &Connection, id: i64) -> Option<String> {
    conn.query_row("SELECT name FROM tags WHERE id = ?1", [id], |r| r.get(0)).ok()
}

/// Log a tag-level event (tag_created / tag_deleted) or a per-track one when `track_id` is set
fn log_tag_event(conn: &Connection, track_id: Option<i64>, kind: &str, tag_id: i64, name: &str) {
    log_or_warn(conn, track_id, kind, &json!({ "tagId": tag_id, "tagName": name }));
}

/// Create a tag, or return the existing one with the same (case-insensitive) name.
/// Returns (tag, created).
pub fn get_or_create_tag(db: &Db, name: &str, color: Option<&str>) -> Result<(DbTag, bool), Error> {
    let conn = db.conn();
    let (id, created) = get_or_create_tag_conn(&conn, name, color)?;
    let tag = get_tag_inner(&conn, id)?;
    if created {
        log_tag_event(&conn, None, "tag_created", id, &tag.name);
    }
    Ok((tag, created))
}

/// Update a tag's name/color/group/keybind. Renaming to an existing (case-insensitive)
/// other tag's name merges this tag into it: track links are re-pointed, this tag is
/// deleted, and the surviving tag is returned. All affected tracks are marked dirty.
pub fn update_tag(
    db: &Db,
    id: i64,
    name: Option<&str>,
    color: Option<&str>,
    group_id: Option<Option<i64>>,
    keybind: Option<Option<Value>>,
) -> Result<DbTag, Error> {
    let conn = db.conn();

    if let Some(name) = name {
        let name = validate_tag_name(name)?;
        match find_tag_by_name(&conn, &name)? {
            // Rename onto another existing tag: merge into it
            Some(other) if other != id => {
                let merged_name = tag_name(&conn, id);
                mark_tag_tracks_dirty(&conn, id)?;
                mark_tag_tracks_dirty(&conn, other)?;
                conn.execute(
                    "INSERT OR IGNORE INTO track_tags (track_id, tag_id, position)
                     SELECT track_id, ?2, position FROM track_tags WHERE tag_id = ?1",
                    params![id, other],
                )?;
                conn.execute("DELETE FROM tags WHERE id = ?1", [id])?;
                if let Some(merged_name) = merged_name {
                    log_tag_event(&conn, None, "tag_deleted", id, &merged_name);
                }
                return get_tag_inner(&conn, other);
            }
            // Plain rename (or case change of itself)
            _ => {
                let from = tag_name(&conn, id);
                mark_tag_tracks_dirty(&conn, id)?;
                conn.execute("UPDATE tags SET name = ?2 WHERE id = ?1", params![id, name])?;
                if let Some(from) = from {
                    log_or_warn(&conn, None, "tag_renamed", &json!({ "tagId": id, "from": from, "to": name }));
                }
            }
        }
    }
    if let Some(color) = color {
        validate_color(color)?;
        conn.execute("UPDATE tags SET color = ?2 WHERE id = ?1", params![id, color])?;
    }
    if let Some(group_id) = group_id {
        if let Some(gid) = group_id {
            let exists: i64 = conn.query_row("SELECT COUNT(*) FROM groups WHERE id = ?1", [gid], |r| r.get(0))?;
            if exists == 0 {
                bail!("Group {gid} does not exist");
            }
        }
        conn.execute("UPDATE tags SET group_id = ?2 WHERE id = ?1", params![id, group_id])?;
    }
    if let Some(keybind) = keybind {
        let json = keybind.map(|v| v.to_string());
        conn.execute("UPDATE tags SET keybind = ?2 WHERE id = ?1", params![id, json])?;
    }
    get_tag_inner(&conn, id)
}

/// Delete a tag; all tracks that carried it are marked dirty
pub fn delete_tag(db: &Db, id: i64) -> Result<(), Error> {
    let conn = db.conn();
    let name = tag_name(&conn, id);
    mark_tag_tracks_dirty(&conn, id)?;
    let n = conn.execute("DELETE FROM tags WHERE id = ?1", [id])?;
    if n == 0 {
        bail!("Tag {id} does not exist");
    }
    if let Some(name) = name {
        log_tag_event(&conn, None, "tag_deleted", id, &name);
    }
    Ok(())
}

/// Mark every track carrying `tag_id` as needing a file flush
fn mark_tag_tracks_dirty(conn: &Connection, tag_id: i64) -> Result<(), Error> {
    conn.execute(
        "UPDATE tracks SET needs_flush = 1, tag_updated_at = ?2, tag_version = tag_version + 1
         WHERE id IN (SELECT track_id FROM track_tags WHERE tag_id = ?1)",
        params![tag_id, now()],
    )?;
    Ok(())
}

fn track_tag_ids(conn: &Connection, track_id: i64) -> Result<Vec<i64>, Error> {
    let mut stmt = conn.prepare("SELECT tag_id FROM track_tags WHERE track_id = ?1 ORDER BY position, tag_id")?;
    let ids = stmt.query_map([track_id], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

/// Add a tag (by name, created if new) to a track.
/// Returns (track's tag ids, newly created tag if any).
pub fn add_tag_to_track(db: &Db, track_id: i64, name: &str) -> Result<(Vec<i64>, Option<DbTag>), Error> {
    let (tag, created) = get_or_create_tag(db, name, None)?;
    let conn = db.conn();
    let pos: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM track_tags WHERE track_id = ?1",
        [track_id],
        |r| r.get(0),
    )?;
    let n = conn.execute(
        "INSERT OR IGNORE INTO track_tags (track_id, tag_id, position) VALUES (?1, ?2, ?3)",
        params![track_id, tag.id, pos],
    )?;
    if n > 0 {
        conn.execute(
            "UPDATE tracks SET needs_flush = 1, tag_updated_at = ?2, tag_version = tag_version + 1 WHERE id = ?1",
            params![track_id, now()],
        )?;
        log_tag_event(&conn, Some(track_id), "tag_added", tag.id, &tag.name);
    }
    Ok((track_tag_ids(&conn, track_id)?, created.then_some(tag)))
}

/// Add a tag (by name, created if new) to MANY tracks in one transaction.
/// Returns (tag, created). Tracks that already carry it are untouched.
pub fn add_tag_to_tracks(db: &Db, track_ids: &[i64], name: &str) -> Result<(DbTag, bool), Error> {
    let name = validate_tag_name(name)?;
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let (tag_id, created) = get_or_create_tag_conn(&tx, &name, None)?;
    let tag_label = tag_name(&tx, tag_id).unwrap_or_else(|| name.clone());
    if created {
        log_tag_event(&tx, None, "tag_created", tag_id, &tag_label);
    }
    let now = now();
    for &track_id in track_ids {
        let pos: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM track_tags WHERE track_id = ?1",
            [track_id],
            |r| r.get(0),
        )?;
        let n = tx.execute(
            "INSERT OR IGNORE INTO track_tags (track_id, tag_id, position) VALUES (?1, ?2, ?3)",
            params![track_id, tag_id, pos],
        )?;
        if n > 0 {
            tx.execute(
                "UPDATE tracks SET needs_flush = 1, tag_updated_at = ?2, tag_version = tag_version + 1 WHERE id = ?1",
                params![track_id, now],
            )?;
            log_tag_event(&tx, Some(track_id), "tag_added", tag_id, &tag_label);
        }
    }
    tx.commit()?;
    Ok((get_tag_inner(&conn, tag_id)?, created))
}

/// Remove a tag from MANY tracks in one transaction
pub fn remove_tag_from_tracks(db: &Db, track_ids: &[i64], tag_id: i64) -> Result<(), Error> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let now = now();
    let tag_label = tag_name(&tx, tag_id).unwrap_or_default();
    for &track_id in track_ids {
        let n = tx.execute(
            "DELETE FROM track_tags WHERE track_id = ?1 AND tag_id = ?2",
            params![track_id, tag_id],
        )?;
        if n > 0 {
            tx.execute(
                "UPDATE tracks SET needs_flush = 1, tag_updated_at = ?2, tag_version = tag_version + 1 WHERE id = ?1",
                params![track_id, now],
            )?;
            log_tag_event(&tx, Some(track_id), "tag_removed", tag_id, &tag_label);
        }
    }
    tx.commit()?;
    Ok(())
}

/// Remove a tag from a track. Returns the track's remaining tag ids.
pub fn remove_tag_from_track(db: &Db, track_id: i64, tag_id: i64) -> Result<Vec<i64>, Error> {
    let conn = db.conn();
    let tag_label = tag_name(&conn, tag_id).unwrap_or_default();
    let n = conn.execute(
        "DELETE FROM track_tags WHERE track_id = ?1 AND tag_id = ?2",
        params![track_id, tag_id],
    )?;
    if n > 0 {
        conn.execute(
            "UPDATE tracks SET needs_flush = 1, tag_updated_at = ?2, tag_version = tag_version + 1 WHERE id = ?1",
            params![track_id, now()],
        )?;
        log_tag_event(&conn, Some(track_id), "tag_removed", tag_id, &tag_label);
    }
    track_tag_ids(&conn, track_id)
}

/// List groups ordered by position
pub fn list_groups(db: &Db) -> Result<Vec<DbGroup>, Error> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT id, name, color, position FROM groups ORDER BY position, id")?;
    let groups = stmt
        .query_map([], |r| {
            Ok(DbGroup { id: r.get(0)?, name: r.get(1)?, color: r.get(2)?, position: r.get(3)? })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(groups)
}

/// Create a group
pub fn create_group(db: &Db, name: &str, color: &str) -> Result<DbGroup, Error> {
    let name = validate_tag_name(name)?;
    validate_color(color)?;
    let conn = db.conn();
    let pos: i64 = conn.query_row("SELECT COALESCE(MAX(position) + 1, 0) FROM groups", [], |r| r.get(0))?;
    conn.execute("INSERT INTO groups (name, color, position) VALUES (?1, ?2, ?3)", params![name, color, pos])?;
    let id = conn.last_insert_rowid();
    Ok(DbGroup { id, name, color: color.to_string(), position: pos })
}

/// Update a group's name/color/position
pub fn update_group(db: &Db, id: i64, name: Option<&str>, color: Option<&str>, position: Option<i64>) -> Result<(), Error> {
    let conn = db.conn();
    if let Some(name) = name {
        let name = validate_tag_name(name)?;
        conn.execute("UPDATE groups SET name = ?2 WHERE id = ?1", params![id, name])?;
    }
    if let Some(color) = color {
        validate_color(color)?;
        conn.execute("UPDATE groups SET color = ?2 WHERE id = ?1", params![id, color])?;
    }
    if let Some(position) = position {
        conn.execute("UPDATE groups SET position = ?2 WHERE id = ?1", params![id, position])?;
    }
    Ok(())
}

/// Delete a group (tags in it fall back to ungrouped via ON DELETE SET NULL)
pub fn delete_group(db: &Db, id: i64) -> Result<(), Error> {
    db.conn().execute("DELETE FROM groups WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{insert_track, test_db};

    #[test]
    fn nocase_dedupe() {
        let (_d, db) = test_db();
        let (a, created_a) = get_or_create_tag(&db, "House", None).unwrap();
        let (b, created_b) = get_or_create_tag(&db, "house", None).unwrap();
        assert!(created_a);
        assert!(!created_b);
        assert_eq!(a.id, b.id);
        assert_eq!(b.name, "House");
    }

    #[test]
    fn add_remove_tag_marks_dirty_and_orders() {
        let (_d, db) = test_db();
        let track = insert_track(&db, "/test-root/a.mp3");
        let (ids, new_tag) = add_tag_to_track(&db, track, "dark").unwrap();
        assert_eq!(ids.len(), 1);
        assert!(new_tag.is_some());
        let (ids, new_tag) = add_tag_to_track(&db, track, "rolling").unwrap();
        assert_eq!(ids.len(), 2);
        assert!(new_tag.is_some());
        // Duplicate add is a no-op
        let (ids, new_tag) = add_tag_to_track(&db, track, "DARK").unwrap();
        assert_eq!(ids.len(), 2);
        assert!(new_tag.is_none());
        // Dirty flag set
        let dirty: i64 = db.conn().query_row("SELECT needs_flush FROM tracks WHERE id = ?1", [track], |r| r.get(0)).unwrap();
        assert_eq!(dirty, 1);
        // Remove
        let remaining = remove_tag_from_track(&db, track, ids[0]).unwrap();
        assert_eq!(remaining.len(), 1);
    }

    #[test]
    fn bulk_add_and_remove() {
        let (_d, db) = test_db();
        let t1 = insert_track(&db, "/test-root/a.mp3");
        let t2 = insert_track(&db, "/test-root/b.mp3");
        let t3 = insert_track(&db, "/test-root/c.mp3");
        // t2 already has the tag: bulk add skips it but tags the others
        add_tag_to_track(&db, t2, "bulk").unwrap();
        let before_v2: i64 = db.conn().query_row("SELECT tag_version FROM tracks WHERE id = ?1", [t2], |r| r.get(0)).unwrap();
        let (tag, created) = add_tag_to_tracks(&db, &[t1, t2, t3], "BULK").unwrap();
        assert!(!created, "case-insensitive match must reuse the tag");
        assert_eq!(tag.count, 3);
        // No version bump for the no-op track
        let after_v2: i64 = db.conn().query_row("SELECT tag_version FROM tracks WHERE id = ?1", [t2], |r| r.get(0)).unwrap();
        assert_eq!(before_v2, after_v2);
        // Bulk remove
        remove_tag_from_tracks(&db, &[t1, t2], tag.id).unwrap();
        assert_eq!(get_tag(&db, tag.id).unwrap().count, 1);
        let dirty: i64 = db.conn().query_row("SELECT COUNT(*) FROM tracks WHERE needs_flush = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(dirty, 3);
    }

    #[test]
    fn delete_tag_cascades_and_marks_dirty() {
        let (_d, db) = test_db();
        let track = insert_track(&db, "/test-root/a.mp3");
        let (ids, _) = add_tag_to_track(&db, track, "temp").unwrap();
        db.conn().execute("UPDATE tracks SET needs_flush = 0", []).unwrap();
        delete_tag(&db, ids[0]).unwrap();
        let conn = db.conn();
        let links: i64 = conn.query_row("SELECT COUNT(*) FROM track_tags", [], |r| r.get(0)).unwrap();
        assert_eq!(links, 0);
        let dirty: i64 = conn.query_row("SELECT needs_flush FROM tracks WHERE id = ?1", [track], |r| r.get(0)).unwrap();
        assert_eq!(dirty, 1);
    }

    #[test]
    fn rename_merge() {
        let (_d, db) = test_db();
        let t1 = insert_track(&db, "/test-root/a.mp3");
        let t2 = insert_track(&db, "/test-root/b.mp3");
        let (_, tag_a) = (0, get_or_create_tag(&db, "Deep", None).unwrap().0);
        let tag_b = get_or_create_tag(&db, "Deep House", None).unwrap().0;
        add_tag_to_track(&db, t1, "Deep").unwrap();
        add_tag_to_track(&db, t2, "Deep").unwrap();
        add_tag_to_track(&db, t2, "Deep House").unwrap();
        // Rename "Deep" -> "deep house" merges into existing tag (case-insensitive)
        let merged = update_tag(&db, tag_a.id, Some("deep house"), None, None, None).unwrap();
        assert_eq!(merged.id, tag_b.id);
        assert_eq!(merged.count, 2);
        let tags = list_tags(&db).unwrap();
        assert_eq!(tags.len(), 1);
    }

    #[test]
    fn merge_appends_positions_after_existing() {
        let (_d, db) = test_db();
        let t = insert_track(&db, "/test-root/a.mp3");
        // Track has "keep" at position 0 and "old" at position 1
        add_tag_to_track(&db, t, "keep").unwrap();
        let old = get_or_create_tag(&db, "old", None).unwrap().0;
        add_tag_to_track(&db, t, "old").unwrap();
        let target = get_or_create_tag(&db, "target", None).unwrap().0;
        // Merging "old" into "target": the merged link inherits the source position
        // (the source link is deleted in the same operation, so no duplicates)
        update_tag(&db, old.id, Some("target"), None, None, None).unwrap();
        let conn = db.conn();
        let mut stmt = conn.prepare("SELECT tag_id, position FROM track_tags WHERE track_id = ?1 ORDER BY position").unwrap();
        let rows: Vec<(i64, i64)> = stmt.query_map([t], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap();
        let positions: Vec<i64> = rows.iter().map(|r| r.1).collect();
        let unique: std::collections::HashSet<i64> = positions.iter().copied().collect();
        assert_eq!(unique.len(), positions.len(), "duplicate positions after merge: {rows:?}");
        assert_eq!(rows.last().unwrap().0, target.id, "merged tag must come last");
    }

    #[test]
    fn group_lifecycle() {
        let (_d, db) = test_db();
        let g = create_group(&db, "Mood", "#ff0000").unwrap();
        let (tag, _) = get_or_create_tag(&db, "happy", None).unwrap();
        update_tag(&db, tag.id, None, None, Some(Some(g.id)), None).unwrap();
        assert_eq!(get_tag(&db, tag.id).unwrap().group_id, Some(g.id));
        // Unknown group rejected
        assert!(update_tag(&db, tag.id, None, None, Some(Some(999)), None).is_err());
        // Deleting the group ungroups the tag
        delete_group(&db, g.id).unwrap();
        assert_eq!(get_tag(&db, tag.id).unwrap().group_id, None);
    }

    #[test]
    fn auto_colors_are_distinct_and_from_palette() {
        let (_d, db) = test_db();
        let names = ["dark", "rolling", "peak", "warm", "deep"];
        let mut colors = Vec::new();
        for name in names {
            let (tag, created) = get_or_create_tag(&db, name, None).unwrap();
            assert!(created);
            assert!(TAG_COLOR_PALETTE.contains(&tag.color.as_str()), "{} not in palette", tag.color);
            assert_ne!(tag.color, DEFAULT_TAG_COLOR, "new tags must not default to teal");
            colors.push(tag.color);
        }
        let unique: std::collections::HashSet<&String> = colors.iter().collect();
        assert_eq!(unique.len(), names.len(), "first {} tags must all differ: {colors:?}", names.len());
    }

    #[test]
    fn auto_color_picks_least_used() {
        let (_d, db) = test_db();
        // Use up the whole palette once
        for i in 0..TAG_COLOR_PALETTE.len() {
            get_or_create_tag(&db, &format!("tag{i}"), None).unwrap();
        }
        // The 13th wraps around to the first palette color
        let (tag, _) = get_or_create_tag(&db, "wrap", None).unwrap();
        assert_eq!(tag.color, TAG_COLOR_PALETTE[0]);
        // An explicitly colored tag doesn't skew the balance for custom colors
        get_or_create_tag(&db, "custom", Some("#123456")).unwrap();
        let (tag2, _) = get_or_create_tag(&db, "wrap2", None).unwrap();
        assert_eq!(tag2.color, TAG_COLOR_PALETTE[1]);
    }

    #[test]
    fn bulk_create_uses_palette() {
        let (_d, db) = test_db();
        let t = insert_track(&db, "/test-root/a.mp3");
        let (tag, created) = add_tag_to_tracks(&db, &[t], "bulkfresh").unwrap();
        assert!(created);
        assert!(TAG_COLOR_PALETTE.contains(&tag.color.as_str()));
        assert_ne!(tag.color, DEFAULT_TAG_COLOR);
    }

    #[test]
    fn invalid_inputs_rejected() {
        let (_d, db) = test_db();
        assert!(get_or_create_tag(&db, "#bad", None).is_err());
        assert!(get_or_create_tag(&db, "ok", Some("red")).is_err());
        assert!(create_group(&db, "", "#ff0000").is_err());
    }

    fn log_rows(db: &Db) -> Vec<(String, Option<i64>, Value)> {
        let conn = db.conn();
        let mut stmt = conn.prepare("SELECT kind, track_id, data FROM edit_log ORDER BY id").unwrap();
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get(1)?, r.get::<_, String>(2)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        rows.into_iter().map(|(k, t, d)| (k, t, serde_json::from_str(&d).unwrap())).collect()
    }

    fn kinds(db: &Db) -> Vec<String> {
        log_rows(db).into_iter().map(|r| r.0).collect()
    }

    #[test]
    fn tag_mutations_write_log() {
        let (_d, db) = test_db();
        let t1 = insert_track(&db, "/test-root/a.mp3");
        let t2 = insert_track(&db, "/test-root/b.mp3");
        let (ids, _) = add_tag_to_track(&db, t1, "dark").unwrap();
        let tag = ids[0];
        let rows = log_rows(&db);
        assert_eq!(kinds(&db), vec!["tag_created", "tag_added"]);
        assert_eq!(rows[0].1, None);
        assert_eq!(rows[0].2, serde_json::json!({"tagId": tag, "tagName": "dark"}));
        assert_eq!(rows[1].1, Some(t1));
        assert_eq!(rows[1].2, serde_json::json!({"tagId": tag, "tagName": "dark"}));
        // duplicate add is a no-op: nothing logged
        add_tag_to_track(&db, t1, "DARK").unwrap();
        assert_eq!(log_rows(&db).len(), 2);
        // bulk add logs per track that actually gained the tag
        add_tag_to_tracks(&db, &[t1, t2], "dark").unwrap();
        assert_eq!(kinds(&db), vec!["tag_created", "tag_added", "tag_added"]);
        // bulk create logs tag_created too
        add_tag_to_tracks(&db, &[t2], "peak").unwrap();
        assert_eq!(kinds(&db)[3..], ["tag_created", "tag_added"]);
        // removes
        remove_tag_from_track(&db, t1, tag).unwrap();
        remove_tag_from_track(&db, t1, tag).unwrap(); // no-op
        remove_tag_from_tracks(&db, &[t1, t2], tag).unwrap(); // only t2 still has it
        let all = log_rows(&db);
        let tail: Vec<_> = all[5..].iter().map(|r| (r.0.as_str(), r.1)).collect();
        assert_eq!(tail, vec![("tag_removed", Some(t1)), ("tag_removed", Some(t2))]);
        // rename, merge, delete
        let n = all.len();
        update_tag(&db, tag, Some("darker"), None, None, None).unwrap();
        let all = log_rows(&db);
        assert_eq!(all[n].0, "tag_renamed");
        assert_eq!(all[n].2, serde_json::json!({"tagId": tag, "from": "dark", "to": "darker"}));
        let other = get_or_create_tag(&db, "keep", None).unwrap().0;
        update_tag(&db, tag, Some("keep"), None, None, None).unwrap(); // merge
        let all = log_rows(&db);
        assert_eq!(all[n + 1].0, "tag_created");
        assert_eq!(all[n + 2].0, "tag_deleted");
        assert_eq!(all[n + 2].2, serde_json::json!({"tagId": tag, "tagName": "darker"}));
        delete_tag(&db, other.id).unwrap();
        let all = log_rows(&db);
        assert_eq!(all.last().unwrap().0, "tag_deleted");
        assert_eq!(all.last().unwrap().2, serde_json::json!({"tagId": other.id, "tagName": "keep"}));
        // deleting a missing tag logs nothing
        let n = all.len();
        assert!(delete_tag(&db, 9999).is_err());
        assert_eq!(log_rows(&db).len(), n);
    }
}

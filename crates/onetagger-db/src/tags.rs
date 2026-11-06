use anyhow::{bail, Error};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::models::{DbGroup, DbTag};
use crate::validate::{validate_color, validate_tag_name};
use crate::{now, Db};

/// Default color for newly created tags
pub const DEFAULT_TAG_COLOR: &str = "#00d2bf";

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

/// Create a tag, or return the existing one with the same (case-insensitive) name.
/// Returns (tag, created).
pub fn get_or_create_tag(db: &Db, name: &str, color: Option<&str>) -> Result<(DbTag, bool), Error> {
    let name = validate_tag_name(name)?;
    let conn = db.conn();
    if let Some(id) = find_tag_by_name(&conn, &name)? {
        return Ok((get_tag_inner(&conn, id)?, false));
    }
    let color = color.unwrap_or(DEFAULT_TAG_COLOR);
    validate_color(color)?;
    conn.execute("INSERT INTO tags (name, color) VALUES (?1, ?2)", params![name, color])?;
    let id = conn.last_insert_rowid();
    Ok((get_tag_inner(&conn, id)?, true))
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
                mark_tag_tracks_dirty(&conn, id)?;
                mark_tag_tracks_dirty(&conn, other)?;
                conn.execute(
                    "INSERT OR IGNORE INTO track_tags (track_id, tag_id, position)
                     SELECT track_id, ?2, position FROM track_tags WHERE tag_id = ?1",
                    params![id, other],
                )?;
                conn.execute("DELETE FROM tags WHERE id = ?1", [id])?;
                return get_tag_inner(&conn, other);
            }
            // Plain rename (or case change of itself)
            _ => {
                mark_tag_tracks_dirty(&conn, id)?;
                conn.execute("UPDATE tags SET name = ?2 WHERE id = ?1", params![id, name])?;
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
    mark_tag_tracks_dirty(&conn, id)?;
    let n = conn.execute("DELETE FROM tags WHERE id = ?1", [id])?;
    if n == 0 {
        bail!("Tag {id} does not exist");
    }
    Ok(())
}

/// Mark every track carrying `tag_id` as needing a file flush
fn mark_tag_tracks_dirty(conn: &Connection, tag_id: i64) -> Result<(), Error> {
    conn.execute(
        "UPDATE tracks SET needs_flush = 1, tag_updated_at = ?2
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
            "UPDATE tracks SET needs_flush = 1, tag_updated_at = ?2 WHERE id = ?1",
            params![track_id, now()],
        )?;
    }
    Ok((track_tag_ids(&conn, track_id)?, created.then_some(tag)))
}

/// Remove a tag from a track. Returns the track's remaining tag ids.
pub fn remove_tag_from_track(db: &Db, track_id: i64, tag_id: i64) -> Result<Vec<i64>, Error> {
    let conn = db.conn();
    let n = conn.execute(
        "DELETE FROM track_tags WHERE track_id = ?1 AND tag_id = ?2",
        params![track_id, tag_id],
    )?;
    if n > 0 {
        conn.execute(
            "UPDATE tracks SET needs_flush = 1, tag_updated_at = ?2 WHERE id = ?1",
            params![track_id, now()],
        )?;
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
    fn invalid_inputs_rejected() {
        let (_d, db) = test_db();
        assert!(get_or_create_tag(&db, "#bad", None).is_err());
        assert!(get_or_create_tag(&db, "ok", Some("red")).is_err());
        assert!(create_group(&db, "", "#ff0000").is_err());
    }
}

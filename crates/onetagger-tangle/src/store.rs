//! Sync bookkeeping: what FlowTag has already sent to Tangle, and the diffs
//! against the current FlowTag tags. Pure SQLite, no HTTP.

use rusqlite::{params, Connection, Result};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A FlowTag tag as Tangle should see it (group resolved to its name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRow {
    pub tag_id: i64,
    pub name: String,
    pub color: String,
    pub group: Option<String>,
}

/// A `tangle_tags` row: a FlowTag tag linked to a Tangle list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linked {
    pub tag_id: i64,
    pub list_id: String,
    pub sent_name: String,
    pub sent_color: Option<String>,
    pub sent_group: Option<String>,
    pub unlinked: bool,
}

/// (tag id, list id, item ids)
pub type Entries = Vec<(i64, String, Vec<String>)>;

const TAG_ROWS: &str = "SELECT t.id, t.name, t.color, g.name FROM tags t LEFT JOIN groups g ON g.id = t.group_id";

fn tag_row(r: &rusqlite::Row) -> Result<TagRow> {
    Ok(TagRow { tag_id: r.get(0)?, name: r.get(1)?, color: r.get(2)?, group: r.get(3)? })
}

fn linked_row(r: &rusqlite::Row) -> Result<Linked> {
    Ok(Linked {
        tag_id: r.get(0)?,
        list_id: r.get(1)?,
        sent_name: r.get(2)?,
        sent_color: r.get(3)?,
        sent_group: r.get(4)?,
        unlinked: r.get::<_, i64>(5)? != 0,
    })
}

const LINKED_COLS: &str = "tag_id, list_id, sent_name, sent_color, sent_group, unlinked";

/// Tags with at least one track matched in Tangle and no Tangle list yet.
pub fn tags_to_create(c: &Connection) -> Result<Vec<TagRow>> {
    let sql = format!(
        "{TAG_ROWS} WHERE t.id NOT IN (SELECT tag_id FROM tangle_tags)
         AND EXISTS (SELECT 1 FROM track_tags tg JOIN tangle_items ti ON ti.track_id = tg.track_id
                     WHERE tg.tag_id = t.id AND ti.item_id IS NOT NULL)
         ORDER BY t.id"
    );
    c.prepare(&sql)?.query_map([], tag_row)?.collect()
}

/// Linked, not unlinked tags whose name, colour or group changed since last sent.
pub fn tags_to_update(c: &Connection) -> Result<Vec<(Linked, TagRow)>> {
    let sql = format!(
        "SELECT tt.tag_id, tt.list_id, tt.sent_name, tt.sent_color, tt.sent_group, tt.unlinked,
                t.id, t.name, t.color, g.name
         FROM tangle_tags tt
         JOIN tags t ON t.id = tt.tag_id
         LEFT JOIN groups g ON g.id = t.group_id
         WHERE tt.unlinked = 0
           AND (t.name COLLATE BINARY IS NOT tt.sent_name
                OR t.color COLLATE BINARY IS NOT tt.sent_color
                OR g.name COLLATE BINARY IS NOT tt.sent_group)
         ORDER BY tt.tag_id"
    );
    let mut stmt = c.prepare(&sql)?;
    let rows = stmt.query_map([], |r| {
        let linked = linked_row(r)?;
        let tag = TagRow { tag_id: r.get(6)?, name: r.get(7)?, color: r.get(8)?, group: r.get(9)? };
        Ok((linked, tag))
    })?;
    rows.collect()
}

/// Linked rows whose FlowTag tag no longer exists.
pub fn tags_to_delete(c: &Connection) -> Result<Vec<Linked>> {
    let sql = format!(
        "SELECT {LINKED_COLS} FROM tangle_tags WHERE tag_id NOT IN (SELECT id FROM tags) ORDER BY tag_id"
    );
    c.prepare(&sql)?.query_map([], linked_row)?.collect()
}

const CURRENT: &str = "SELECT tt.tag_id, tt.list_id, ti.item_id
    FROM track_tags tg
    JOIN tangle_items ti ON ti.track_id = tg.track_id AND ti.item_id IS NOT NULL
    JOIN tangle_tags tt ON tt.tag_id = tg.tag_id AND tt.unlinked = 0";

const SENT: &str = "SELECT tt.tag_id, tt.list_id, s.item_id
    FROM tangle_sent s
    JOIN tangle_tags tt ON tt.tag_id = s.tag_id AND tt.unlinked = 0";

fn grouped(c: &Connection, sql: &str) -> Result<Entries> {
    let mut stmt = c.prepare(sql)?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
    let mut out: Entries = Vec::new();
    for row in rows {
        let (tag, list, item) = row?;
        match out.last_mut() {
            Some((t, _, items)) if *t == tag => items.push(item),
            _ => out.push((tag, list, vec![item])),
        }
    }
    Ok(out)
}

/// (tag, item) pairs currently tagged in FlowTag but not yet sent.
pub fn entries_to_add(c: &Connection) -> Result<Entries> {
    grouped(c, &format!("{CURRENT} EXCEPT {SENT} ORDER BY 1, 3"))
}

/// (tag, item) pairs sent earlier that FlowTag no longer has.
pub fn entries_to_remove(c: &Connection) -> Result<Entries> {
    grouped(c, &format!("{SENT} EXCEPT {CURRENT} ORDER BY 1, 3"))
}

/// Record a freshly created Tangle list for a tag (replaces any previous link).
pub fn link_tag(c: &Connection, tag: &TagRow, list_id: &str) -> Result<()> {
    c.execute(
        "INSERT OR REPLACE INTO tangle_tags (tag_id, list_id, sent_name, sent_color, sent_group, unlinked)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        params![tag.tag_id, list_id, tag.name, tag.color, tag.group],
    )?;
    Ok(())
}

/// Record that name/colour/group were pushed to Tangle.
pub fn mark_updated(c: &Connection, tag: &TagRow) -> Result<()> {
    c.execute(
        "UPDATE tangle_tags SET sent_name = ?2, sent_color = ?3, sent_group = ?4 WHERE tag_id = ?1",
        params![tag.tag_id, tag.name, tag.color, tag.group],
    )?;
    Ok(())
}

/// Drop all bookkeeping for a tag (link and sent entries).
pub fn forget_tag(c: &Connection, tag_id: i64) -> Result<()> {
    c.execute("DELETE FROM tangle_sent WHERE tag_id = ?1", [tag_id])?;
    c.execute("DELETE FROM tangle_tags WHERE tag_id = ?1", [tag_id])?;
    Ok(())
}

/// The list was deleted in Tangle: stop syncing the tag until the user resends.
pub fn mark_unlinked(c: &Connection, tag_id: i64) -> Result<()> {
    c.execute("UPDATE tangle_tags SET unlinked = 1 WHERE tag_id = ?1", [tag_id])?;
    c.execute("DELETE FROM tangle_sent WHERE tag_id = ?1", [tag_id])?;
    Ok(())
}

/// User pressed Send again: forget the tag so it is created afresh.
pub fn resend_tag(c: &Connection, tag_id: i64) -> Result<()> {
    forget_tag(c, tag_id)
}

pub fn record_sent(c: &Connection, tag_id: i64, items: &[String]) -> Result<()> {
    let mut stmt = c.prepare("INSERT OR IGNORE INTO tangle_sent (tag_id, item_id) VALUES (?1, ?2)")?;
    for item in items {
        stmt.execute(params![tag_id, item])?;
    }
    Ok(())
}

pub fn record_unsent(c: &Connection, tag_id: i64, items: &[String]) -> Result<()> {
    let mut stmt = c.prepare("DELETE FROM tangle_sent WHERE tag_id = ?1 AND item_id = ?2")?;
    for item in items {
        stmt.execute(params![tag_id, item])?;
    }
    Ok(())
}

/// Cheap change detector: tracks' `tag_version` sum combined with a hash of the tag definitions (including group names).
pub fn fingerprint(c: &Connection) -> Result<i64> {
    let versions: i64 =
        c.query_row("SELECT COALESCE(SUM(tag_version), 0) FROM tracks", [], |r| r.get(0))?;
    let mut hasher = DefaultHasher::new();
    let mut stmt = c.prepare(
        "SELECT t.id, t.name, t.color, t.group_id, g.name FROM tags t
         LEFT JOIN groups g ON g.id = t.group_id ORDER BY t.id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    })?;
    for row in rows {
        row?.hash(&mut hasher);
    }
    Ok(versions.wrapping_add(hasher.finish() as i64))
}

/// Tags the user deleted in Tangle (waiting for Send again): (tag id, display name).
pub fn unlinked_tags(c: &Connection) -> Result<Vec<(i64, String)>> {
    c.prepare(
        "SELECT tt.tag_id, COALESCE(t.name, tt.sent_name) FROM tangle_tags tt
         LEFT JOIN tags t ON t.id = tt.tag_id WHERE tt.unlinked = 1 ORDER BY tt.tag_id",
    )?
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
    .collect()
}

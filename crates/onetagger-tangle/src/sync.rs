//! One sync pass: push FlowTag tag changes to Tangle.
//! Order: delete lists, update lists, create lists, remove entries, add entries.
//! Each phase recomputes its diff; the DB is updated after every confirmed request
//! and the connection lock is never held across HTTP.

use crate::client::{Client, ListPatch};
use crate::error::TangleError;
use crate::matcher::db_error;
use crate::store::{self, Entries, Linked, TagRow};
use onetagger_db::Db;
use rusqlite::Connection;
use std::time::Duration;

const BATCH: usize = 100;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PassReport {
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
    pub added: usize,
    pub removed: usize,
    /// (tag, item) adds + removes still outstanding after the pass.
    pub pending: usize,
    /// Tags deleted in Tangle, waiting for the user to send again.
    pub unlinked: Vec<(i64, String)>,
}

type Res<T> = Result<T, TangleError>;

fn with_db<T>(db: &Db, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Res<T> {
    let c = db.conn();
    f(&c).map_err(db_error)
}

/// Sleeps `pace` between consecutive requests.
struct Pacer {
    pace: Duration,
    started: bool,
}

impl Pacer {
    fn wait(&mut self) {
        if self.started {
            std::thread::sleep(self.pace);
        }
        self.started = true;
    }
}

#[derive(PartialEq)]
enum Flow {
    Continue,
    ListGone,
}

pub fn run_pass(db: &Db, client: &Client, pace: Duration) -> Res<PassReport> {
    let mut p = Pacer { pace, started: false };
    let mut report = PassReport::default();
    delete_lists(db, client, &mut p, &mut report)?;
    update_lists(db, client, &mut p, &mut report)?;
    create_lists(db, client, &mut p, &mut report)?;
    send_entries(db, client, &mut p, &mut report, true)?;
    send_entries(db, client, &mut p, &mut report, false)?;
    report.pending = with_db(db, |c| {
        let n = |e: Entries| e.iter().map(|(_, _, i)| i.len()).sum::<usize>();
        Ok(n(store::entries_to_add(c)?) + n(store::entries_to_remove(c)?))
    })?;
    report.unlinked = with_db(db, store::unlinked_tags)?;
    Ok(report)
}

fn delete_lists(db: &Db, client: &Client, p: &mut Pacer, rep: &mut PassReport) -> Res<()> {
    for row in with_db(db, store::tags_to_delete)? {
        p.wait();
        match client.delete_list(&row.list_id) {
            Ok(()) | Err(TangleError::ListNotFound) => {
                with_db(db, |c| store::forget_tag(c, row.tag_id))?;
                rep.deleted += 1;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn update_lists(db: &Db, client: &Client, p: &mut Pacer, rep: &mut PassReport) -> Res<()> {
    for (linked, tag) in with_db(db, store::tags_to_update)? {
        p.wait();
        match client.update_list(&linked.list_id, &changed_fields(&linked, &tag)) {
            Ok(()) => {
                with_db(db, |c| store::mark_updated(c, &tag))?;
                rep.updated += 1;
            }
            Err(TangleError::ListNotFound) => with_db(db, |c| store::mark_unlinked(c, tag.tag_id))?,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Only what FlowTag changed since the last send: Tangle-side edits to other fields stay.
fn changed_fields<'a>(sent: &Linked, tag: &'a TagRow) -> ListPatch<'a> {
    ListPatch {
        name: (tag.name != sent.sent_name).then_some(tag.name.as_str()),
        color: (sent.sent_color.as_deref() != Some(tag.color.as_str())).then_some(tag.color.as_str()),
        group: (tag.group != sent.sent_group).then_some(tag.group.as_deref()),
    }
}

fn create_lists(db: &Db, client: &Client, p: &mut Pacer, rep: &mut PassReport) -> Res<()> {
    for tag in with_db(db, store::tags_to_create)? {
        p.wait();
        let id = client.create_list(&tag.name, Some(&tag.color), tag.group.as_deref())?;
        with_db(db, |c| store::link_tag(c, &tag, &id))?;
        rep.created += 1;
    }
    Ok(())
}

fn send_entries(db: &Db, client: &Client, p: &mut Pacer, rep: &mut PassReport, remove: bool) -> Res<()> {
    let entries = with_db(db, |c| {
        if remove { store::entries_to_remove(c) } else { store::entries_to_add(c) }
    })?;
    for (tag_id, list, items) in entries {
        for chunk in items.chunks(BATCH) {
            let done = send_batch(db, client, p, (tag_id, &list), chunk, remove)?;
            *(if remove { &mut rep.removed } else { &mut rep.added }) += done.0;
            if done.1 == Flow::ListGone {
                break;
            }
        }
    }
    Ok(())
}

fn request(client: &Client, list: &str, items: &[String], remove: bool) -> Res<()> {
    if remove { client.remove_entries(list, items) } else { client.add_entries(list, items) }
}

fn record(db: &Db, tag_id: i64, items: &[String], remove: bool) -> Res<()> {
    with_db(db, |c| {
        if remove { store::record_unsent(c, tag_id, items) } else { store::record_sent(c, tag_id, items) }
    })
}

/// Returns (confirmed item count, flow).
fn send_batch(
    db: &Db,
    client: &Client,
    p: &mut Pacer,
    (tag_id, list): (i64, &str),
    chunk: &[String],
    remove: bool,
) -> Res<(usize, Flow)> {
    p.wait();
    match request(client, list, chunk, remove) {
        Ok(()) => {
            record(db, tag_id, chunk, remove)?;
            Ok((chunk.len(), Flow::Continue))
        }
        Err(TangleError::ListNotFound) => {
            with_db(db, |c| store::mark_unlinked(c, tag_id))?;
            Ok((0, Flow::ListGone))
        }
        Err(TangleError::ItemNotFound) => send_singly(db, client, p, (tag_id, list), chunk, remove),
        Err(e) => Err(e),
    }
}

/// A batch had an unknown item: find it by sending one at a time.
fn send_singly(
    db: &Db,
    client: &Client,
    p: &mut Pacer,
    (tag_id, list): (i64, &str),
    chunk: &[String],
    remove: bool,
) -> Res<(usize, Flow)> {
    let mut done = 0;
    for item in chunk {
        p.wait();
        let one = std::slice::from_ref(item);
        match request(client, list, one, remove) {
            Ok(()) => {
                record(db, tag_id, one, remove)?;
                done += 1;
            }
            Err(TangleError::ItemNotFound) => {
                with_db(db, |c| {
                    c.execute(
                        "UPDATE tangle_items SET item_id = NULL, checked_at = 0 WHERE item_id = ?1",
                        [item],
                    )?;
                    Ok(())
                })?;
                if remove {
                    record(db, tag_id, one, true)?;
                }
            }
            Err(TangleError::ListNotFound) => {
                with_db(db, |c| store::mark_unlinked(c, tag_id))?;
                return Ok((done, Flow::ListGone));
            }
            Err(e) => return Err(e),
        }
    }
    Ok((done, Flow::Continue))
}

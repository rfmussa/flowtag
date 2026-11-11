mod common;
use common::*;
use onetagger_tangle::store::*;

fn s(v: &str) -> String { v.to_string() }

fn tag_row(c: &rusqlite::Connection, id: i64) -> TagRow {
    tags_to_create(c).unwrap().into_iter().find(|t| t.tag_id == id).expect("tag row")
}

#[test]
fn new_tag_with_matched_track_is_created_empty_tag_is_not() {
    let t = TestDb::new();
    let c = t.db.conn();
    let tr = add_track(&c, "a");
    set_item(&c, tr, Some("itm_1"));
    let used = add_tag(&c, "dark", "#111111", None);
    let _empty = add_tag(&c, "empty", "#222222", None);
    tag_track(&c, tr, used);
    let created = tags_to_create(&c).unwrap();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].tag_id, used);
    assert_eq!(created[0].name, "dark");
    assert_eq!(created[0].group, None);
}

#[test]
fn untag_yields_remove_tag_yields_add() {
    let t = TestDb::new();
    let c = t.db.conn();
    let tr = add_track(&c, "a");
    set_item(&c, tr, Some("itm_1"));
    let tag = add_tag(&c, "dark", "#111111", None);
    tag_track(&c, tr, tag);
    link_tag(&c, &tag_row(&c, tag), "lst_1").unwrap();
    assert_eq!(entries_to_add(&c).unwrap(), vec![(tag, s("lst_1"), vec![s("itm_1")])]);
    record_sent(&c, tag, &[s("itm_1")]).unwrap();
    assert!(entries_to_add(&c).unwrap().is_empty());
    assert!(entries_to_remove(&c).unwrap().is_empty());

    untag_track(&c, tr, tag);
    assert_eq!(entries_to_remove(&c).unwrap(), vec![(tag, s("lst_1"), vec![s("itm_1")])]);
    record_unsent(&c, tag, &[s("itm_1")]).unwrap();
    assert!(entries_to_remove(&c).unwrap().is_empty());

    tag_track(&c, tr, tag);
    assert_eq!(entries_to_add(&c).unwrap().len(), 1);
    record_sent(&c, tag, &[s("itm_1")]).unwrap();
    assert!(entries_to_add(&c).unwrap().is_empty() && entries_to_remove(&c).unwrap().is_empty());
}

#[test]
fn rename_recolor_regroup_yield_update() {
    let t = TestDb::new();
    let c = t.db.conn();
    let tr = add_track(&c, "a");
    set_item(&c, tr, Some("itm_1"));
    let tag = add_tag(&c, "dark", "#111111", None);
    tag_track(&c, tr, tag);
    link_tag(&c, &tag_row(&c, tag), "lst_1").unwrap();
    assert!(tags_to_update(&c).unwrap().is_empty());

    c.execute("UPDATE tags SET name = 'darker' WHERE id = ?1", [tag]).unwrap();
    let u = tags_to_update(&c).unwrap();
    assert_eq!(u.len(), 1);
    assert_eq!(u[0].0.sent_name, "dark");
    assert_eq!(u[0].1.name, "darker");
    mark_updated(&c, &u[0].1).unwrap();
    assert!(tags_to_update(&c).unwrap().is_empty());

    c.execute("UPDATE tags SET color = '#999999' WHERE id = ?1", [tag]).unwrap();
    assert_eq!(tags_to_update(&c).unwrap().len(), 1);
    mark_updated(&c, &tags_to_update(&c).unwrap()[0].1).unwrap();

    let g = add_group(&c, "mood");
    c.execute("UPDATE tags SET group_id = ?1 WHERE id = ?2", [g, tag]).unwrap();
    let u = tags_to_update(&c).unwrap();
    assert_eq!(u[0].1.group.as_deref(), Some("mood"));
    mark_updated(&c, &u[0].1).unwrap();
    assert!(tags_to_update(&c).unwrap().is_empty());
}

#[test]
fn deleted_tag_yields_list_delete() {
    let t = TestDb::new();
    let c = t.db.conn();
    let tr = add_track(&c, "a");
    set_item(&c, tr, Some("itm_1"));
    let tag = add_tag(&c, "dark", "#111111", None);
    tag_track(&c, tr, tag);
    link_tag(&c, &tag_row(&c, tag), "lst_1").unwrap();
    record_sent(&c, tag, &[s("itm_1")]).unwrap();
    assert!(tags_to_delete(&c).unwrap().is_empty());
    c.execute("DELETE FROM tags WHERE id = ?1", [tag]).unwrap();
    let d = tags_to_delete(&c).unwrap();
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].list_id, "lst_1");
    forget_tag(&c, tag).unwrap();
    assert!(tags_to_delete(&c).unwrap().is_empty());
    let sent: i64 = c.query_row("SELECT COUNT(*) FROM tangle_sent", [], |r| r.get(0)).unwrap();
    assert_eq!(sent, 0);
}

#[test]
fn tangle_side_entries_never_appear() {
    let t = TestDb::new();
    let c = t.db.conn();
    let tr = add_track(&c, "a");
    set_item(&c, tr, Some("itm_1"));
    let tag = add_tag(&c, "dark", "#111111", None);
    link_tag(&c, &TagRow { tag_id: tag, name: s("dark"), color: s("#111111"), group: None }, "lst_1").unwrap();
    // Track untagged in FlowTag, nothing sent: Tangle may hold other entries, we never see them.
    assert!(entries_to_add(&c).unwrap().is_empty());
    assert!(entries_to_remove(&c).unwrap().is_empty());
}

#[test]
fn removed_track_keeps_entry_if_another_track_shares_item() {
    let t = TestDb::new();
    let c = t.db.conn();
    let t1 = add_track(&c, "a");
    let t2 = add_track(&c, "b");
    set_item(&c, t1, Some("itm_1"));
    set_item(&c, t2, Some("itm_1"));
    let tag = add_tag(&c, "dark", "#111111", None);
    tag_track(&c, t1, tag);
    tag_track(&c, t2, tag);
    link_tag(&c, &tag_row(&c, tag), "lst_1").unwrap();
    record_sent(&c, tag, &[s("itm_1")]).unwrap();
    c.execute("DELETE FROM tracks WHERE id = ?1", [t1]).unwrap();
    assert!(entries_to_remove(&c).unwrap().is_empty());
    c.execute("DELETE FROM tracks WHERE id = ?1", [t2]).unwrap();
    assert_eq!(entries_to_remove(&c).unwrap(), vec![(tag, s("lst_1"), vec![s("itm_1")])]);
}

#[test]
fn unlinked_tag_is_skipped() {
    let t = TestDb::new();
    let c = t.db.conn();
    let tr = add_track(&c, "a");
    let tr2 = add_track(&c, "b");
    set_item(&c, tr, Some("itm_1"));
    set_item(&c, tr2, Some("itm_2"));
    let tag = add_tag(&c, "dark", "#111111", None);
    tag_track(&c, tr, tag);
    link_tag(&c, &tag_row(&c, tag), "lst_1").unwrap();
    record_sent(&c, tag, &[s("itm_1")]).unwrap();
    mark_unlinked(&c, tag).unwrap();
    // Still tagged (itm_1) plus a newly tagged track (itm_2), with leftover sent rows.
    tag_track(&c, tr2, tag);
    c.execute("INSERT INTO tangle_sent (tag_id, item_id) VALUES (?1, 'itm_gone')", [tag]).unwrap();
    c.execute("UPDATE tags SET name = 'x' WHERE id = ?1", [tag]).unwrap();
    assert!(entries_to_add(&c).unwrap().is_empty());
    assert!(entries_to_remove(&c).unwrap().is_empty());
    assert!(tags_to_update(&c).unwrap().is_empty());
    assert!(tags_to_create(&c).unwrap().is_empty());
    // Pressing Send again: resend_tag forgets the link; the tag is creatable afresh.
    resend_tag(&c, tag).unwrap();
    assert_eq!(tags_to_create(&c).unwrap().len(), 1);
    let sent: i64 = c.query_row("SELECT COUNT(*) FROM tangle_sent", [], |r| r.get(0)).unwrap();
    assert_eq!(sent, 0);
}

#[test]
fn case_only_rename_yields_update() {
    let t = TestDb::new();
    let c = t.db.conn();
    let tr = add_track(&c, "a");
    set_item(&c, tr, Some("itm_1"));
    let g = add_group(&c, "mood");
    let tag = add_tag(&c, "dark", "#111111", Some(g));
    tag_track(&c, tr, tag);
    link_tag(&c, &tag_row(&c, tag), "lst_1").unwrap();
    c.execute("UPDATE tags SET name = 'Dark' WHERE id = ?1", [tag]).unwrap();
    let u = tags_to_update(&c).unwrap();
    assert_eq!(u.len(), 1);
    assert_eq!(u[0].1.name, "Dark");
    mark_updated(&c, &u[0].1).unwrap();
    assert!(tags_to_update(&c).unwrap().is_empty());
    c.execute("UPDATE groups SET name = 'Mood' WHERE id = ?1", [g]).unwrap();
    assert_eq!(tags_to_update(&c).unwrap().len(), 1);
}

#[test]
fn fingerprint_changes_on_tag_untag_rename() {
    let t = TestDb::new();
    let c = t.db.conn();
    let tr = add_track(&c, "a");
    let tag = add_tag(&c, "dark", "#111111", None);
    let f0 = fingerprint(&c).unwrap();
    tag_track(&c, tr, tag);
    let f1 = fingerprint(&c).unwrap();
    assert_ne!(f0, f1);
    c.execute("UPDATE tags SET name = 'darker' WHERE id = ?1", [tag]).unwrap();
    let f2 = fingerprint(&c).unwrap();
    assert_ne!(f1, f2);
    untag_track(&c, tr, tag);
    assert_ne!(f2, fingerprint(&c).unwrap());
    assert_eq!(fingerprint(&c).unwrap(), fingerprint(&c).unwrap());
}

#[test]
fn group_rename_changes_fingerprint() {
    let t = TestDb::new();
    let c = t.db.conn();
    let g = add_group(&c, "mood");
    add_tag(&c, "dark", "#111111", Some(g));
    let before = fingerprint(&c).unwrap();
    c.execute("UPDATE groups SET name = 'vibe' WHERE id = ?1", [g]).unwrap();
    assert_ne!(before, fingerprint(&c).unwrap());
}

#[test]
fn unlinked_tags_lists_name_or_sent_name() {
    let t = TestDb::new();
    let c = t.db.conn();
    let tr = add_track(&c, "a");
    set_item(&c, tr, Some("itm_1"));
    let kept = add_tag(&c, "dark", "#111111", None);
    let gone = add_tag(&c, "old", "#222222", None);
    let linked = add_tag(&c, "fine", "#333333", None);
    for tag in [kept, gone, linked] {
        tag_track(&c, tr, tag);
    }
    for (tag, list) in [(kept, "lst_1"), (gone, "lst_2"), (linked, "lst_3")] {
        link_tag(&c, &tag_row(&c, tag), list).unwrap();
    }
    mark_unlinked(&c, kept).unwrap();
    mark_unlinked(&c, gone).unwrap();
    c.execute("DELETE FROM tags WHERE id = ?1", [gone]).unwrap();
    assert_eq!(
        unlinked_tags(&c).unwrap(),
        vec![(kept, "dark".to_string()), (gone, "old".to_string())]
    );
}

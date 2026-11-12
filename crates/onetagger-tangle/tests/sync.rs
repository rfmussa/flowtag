mod common;
use common::*;
use mockito::{Matcher, Mock, Server, ServerGuard};
use onetagger_tangle::client::Client;
use onetagger_tangle::error::TangleError;
use onetagger_tangle::sync::run_pass;
use std::time::Duration;

fn pass(t: &TestDb, server: &ServerGuard) -> Result<onetagger_tangle::sync::PassReport, TangleError> {
    run_pass(&t.db, &Client::new(&server.url(), "tok"), Duration::ZERO)
}

fn mock_create(s: &mut Server, id: &str) -> Mock {
    s.mock("POST", "/lists").with_body(format!(r#"{{"id":"{id}"}}"#)).expect(1).create()
}

fn mock_ok(s: &mut Server, method: &str, path: &str) -> Mock {
    s.mock(method, path).with_body("{}").create()
}

/// One tagged, matched track; returns (track, tag).
fn setup_one(t: &TestDb) -> (i64, i64) {
    let c = t.db.conn();
    let tr = add_track(&c, "a");
    set_item(&c, tr, Some("trk_1"));
    let tag = add_tag(&c, "dark", "#111111", None);
    tag_track(&c, tr, tag);
    (tr, tag)
}

#[test]
fn first_pass_creates_tags_and_adds_entries() {
    let mut s = Server::new();
    let t = TestDb::new();
    setup_one(&t);
    let create = mock_create(&mut s, "lst_1");
    let add = s
        .mock("POST", "/lists/lst_1/entries")
        .match_body(Matcher::PartialJsonString(r#"{"items":["trk_1"]}"#.into()))
        .with_body("{}")
        .expect(1)
        .create();
    let r = pass(&t, &s).unwrap();
    create.assert();
    add.assert();
    assert_eq!((r.created, r.added, r.pending), (1, 1, 0));
}

#[test]
fn second_identical_pass_sends_nothing() {
    let mut s = Server::new();
    let t = TestDb::new();
    setup_one(&t);
    let create = mock_create(&mut s, "lst_1");
    let add = s.mock("POST", "/lists/lst_1/entries").with_body("{}").expect(1).create();
    pass(&t, &s).unwrap();
    let r = pass(&t, &s).unwrap();
    create.assert();
    add.assert();
    assert_eq!((r.created, r.updated, r.deleted, r.added, r.removed, r.pending), (0, 0, 0, 0, 0, 0));
}

#[test]
fn untag_removes_entry() {
    let mut s = Server::new();
    let t = TestDb::new();
    let (tr, tag) = setup_one(&t);
    mock_create(&mut s, "lst_1");
    mock_ok(&mut s, "POST", "/lists/lst_1/entries");
    pass(&t, &s).unwrap();
    untag_track(&t.db.conn(), tr, tag);
    let rm = s
        .mock("DELETE", "/lists/lst_1/entries")
        .match_body(Matcher::PartialJsonString(r#"{"items":["trk_1"]}"#.into()))
        .with_body("{}")
        .expect(1)
        .create();
    let r = pass(&t, &s).unwrap();
    rm.assert();
    assert_eq!((r.removed, r.pending), (1, 0));
    let n: i64 = t.db.conn().query_row("SELECT COUNT(*) FROM tangle_sent", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn rename_patches_list() {
    let mut s = Server::new();
    let t = TestDb::new();
    let (_, tag) = setup_one(&t);
    mock_create(&mut s, "lst_1");
    mock_ok(&mut s, "POST", "/lists/lst_1/entries");
    pass(&t, &s).unwrap();
    onetagger_db::tags::update_tag(&t.db, tag, Some("darker"), None, None, None).unwrap();
    let patch = s
        .mock("PATCH", "/lists/lst_1")
        .match_body(Matcher::PartialJsonString(r#"{"name":"darker"}"#.into()))
        .with_body("{}")
        .expect(1)
        .create();
    let r = pass(&t, &s).unwrap();
    patch.assert();
    assert_eq!(r.updated, 1);
    assert_eq!(pass(&t, &s).unwrap().updated, 0);
}

#[test]
fn delete_removes_list() {
    let mut s = Server::new();
    let t = TestDb::new();
    let (_, tag) = setup_one(&t);
    mock_create(&mut s, "lst_1");
    mock_ok(&mut s, "POST", "/lists/lst_1/entries");
    pass(&t, &s).unwrap();
    t.db.conn().execute("DELETE FROM tags WHERE id = ?1", [tag]).unwrap();
    let del = mock_ok(&mut s, "DELETE", "/lists/lst_1").expect(1).create();
    let r = pass(&t, &s).unwrap();
    del.assert();
    assert_eq!(r.deleted, 1);
    let n: i64 = t.db.conn().query_row("SELECT COUNT(*) FROM tangle_tags", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
}

fn setup_many(t: &TestDb, n: usize) {
    let c = t.db.conn();
    let tag = add_tag(&c, "dark", "#111111", None);
    for i in 0..n {
        let tr = add_track(&c, &format!("t{i}"));
        set_item(&c, tr, Some(&format!("itm_{i:03}")));
        tag_track(&c, tr, tag);
    }
}

fn body_items(req: &mockito::Request) -> usize {
    let v: serde_json::Value = serde_json::from_slice(req.body().unwrap()).unwrap();
    v["items"].as_array().unwrap().len()
}

#[test]
fn batches_of_100() {
    let mut s = Server::new();
    let t = TestDb::new();
    setup_many(&t, 250);
    mock_create(&mut s, "lst_1");
    let full = s
        .mock("POST", "/lists/lst_1/entries")
        .match_request(|r| body_items(r) == 100)
        .with_body("{}")
        .expect(2)
        .create();
    let rest = s
        .mock("POST", "/lists/lst_1/entries")
        .match_request(|r| body_items(r) == 50)
        .with_body("{}")
        .expect(1)
        .create();
    let r = pass(&t, &s).unwrap();
    full.assert();
    rest.assert();
    assert_eq!((r.added, r.pending), (250, 0));
}

#[test]
fn list_not_found_marks_unlinked_and_is_not_recreated() {
    let mut s = Server::new();
    let t = TestDb::new();
    let (_, tag) = setup_one(&t);
    let create = mock_create(&mut s, "lst_1");
    s.mock("POST", "/lists/lst_1/entries")
        .with_status(404)
        .with_body(r#"{"error":{"code":"list_not_found","message":"x"}}"#)
        .create();
    let r = pass(&t, &s).unwrap();
    assert_eq!(r.unlinked, vec![(tag, "dark".to_string())]);
    assert_eq!(r.pending, 0);
    let r2 = pass(&t, &s).unwrap();
    create.assert(); // still exactly one POST /lists
    assert_eq!(r2.created, 0);
    assert_eq!(r2.unlinked.len(), 1);
}

#[test]
fn merge_deletes_old_list_and_moves_entries() {
    let mut s = Server::new();
    let t = TestDb::new();
    let (a, b, old, keep);
    {
        let c = t.db.conn();
        a = add_track(&c, "a");
        b = add_track(&c, "b");
        set_item(&c, a, Some("itm_a"));
        set_item(&c, b, Some("itm_b"));
        old = add_tag(&c, "old", "#111111", None);
        keep = add_tag(&c, "keep", "#222222", None);
        tag_track(&c, a, old);
        tag_track(&c, b, keep);
    }
    s.mock("POST", "/lists")
        .match_body(Matcher::PartialJsonString(r#"{"name":"old"}"#.into()))
        .with_body(r#"{"id":"lst_old"}"#)
        .create();
    s.mock("POST", "/lists")
        .match_body(Matcher::PartialJsonString(r#"{"name":"keep"}"#.into()))
        .with_body(r#"{"id":"lst_keep"}"#)
        .create();
    mock_ok(&mut s, "POST", "/lists/lst_old/entries");
    mock_ok(&mut s, "POST", "/lists/lst_keep/entries");
    pass(&t, &s).unwrap();

    onetagger_db::tags::update_tag(&t.db, old, Some("keep"), None, None, None).unwrap();
    let del = mock_ok(&mut s, "DELETE", "/lists/lst_old").expect(1).create();
    let moved = s
        .mock("POST", "/lists/lst_keep/entries")
        .match_body(Matcher::PartialJsonString(r#"{"items":["itm_a"]}"#.into()))
        .with_body("{}")
        .expect(1)
        .create();
    let r = pass(&t, &s).unwrap();
    del.assert();
    moved.assert();
    assert_eq!((r.deleted, r.added, r.pending), (1, 1, 0));
}

#[test]
fn unchanged_entries_count_as_sent() {
    let mut s = Server::new();
    let t = TestDb::new();
    setup_one(&t);
    mock_create(&mut s, "lst_1");
    s.mock("POST", "/lists/lst_1/entries").with_body(r#"{"unchanged":["trk_1"]}"#).expect(1).create();
    let r = pass(&t, &s).unwrap();
    assert_eq!((r.added, r.pending), (1, 0));
    let n: i64 = t.db.conn().query_row("SELECT COUNT(*) FROM tangle_sent", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}

#[test]
fn rate_limit_midway_keeps_confirmed_batches() {
    let mut s = Server::new();
    let t = TestDb::new();
    setup_many(&t, 250);
    let create = mock_create(&mut s, "lst_1");
    let b1 = s.mock("POST", "/lists/lst_1/entries").match_body(Matcher::Regex("itm_000".into())).with_body("{}").expect(1).create();
    let limited = s
        .mock("POST", "/lists/lst_1/entries")
        .match_body(Matcher::Regex("itm_100".into()))
        .with_status(429)
        .with_header("Retry-After", "7")
        .with_body(r#"{"error":{"code":"rate_limited","message":"x"}}"#)
        .expect(1)
        .create();
    let err = pass(&t, &s).unwrap_err();
    assert_eq!(err, TangleError::RateLimited { retry_after_s: 7 });
    b1.assert();
    limited.assert();
    limited.remove();

    let b2 = s.mock("POST", "/lists/lst_1/entries").match_body(Matcher::Regex("itm_100".into())).with_body("{}").expect(1).create();
    let b3 = s.mock("POST", "/lists/lst_1/entries").match_body(Matcher::Regex("itm_200".into())).with_body("{}").expect(1).create();
    let r = pass(&t, &s).unwrap();
    b1.assert(); // still just the one hit from pass 1
    b2.assert();
    b3.assert();
    create.assert();
    assert_eq!((r.created, r.added, r.pending), (0, 150, 0));
}

#[test]
fn item_not_found_retries_singly_and_clears_item() {
    let mut s = Server::new();
    let t = TestDb::new();
    {
        let c = t.db.conn();
        let tag = add_tag(&c, "dark", "#111111", None);
        for (n, item) in [("a", "itm_good"), ("b", "itm_bad")] {
            let tr = add_track(&c, n);
            set_item(&c, tr, Some(item));
            tag_track(&c, tr, tag);
        }
    }
    mock_create(&mut s, "lst_1");
    let not_found = r#"{"error":{"code":"item_not_found","message":"x"}}"#;
    let bad = s.mock("POST", "/lists/lst_1/entries").match_body(Matcher::Regex("itm_bad".into())).with_status(404).with_body(not_found).expect(2).create();
    let good = s.mock("POST", "/lists/lst_1/entries").match_body(Matcher::Regex(r#"^\{"items":\["itm_good"\]\}$"#.into())).with_body("{}").expect(1).create();
    let r = pass(&t, &s).unwrap();
    bad.assert();
    good.assert();
    assert_eq!((r.added, r.pending), (1, 0));
    let cleared: Option<String> = t.db.conn()
        .query_row("SELECT item_id FROM tangle_items WHERE track_id = (SELECT id FROM tracks WHERE title = 'b')", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cleared, None);
}

fn patch_body(req: &mockito::Request) -> serde_json::Value {
    serde_json::from_slice(req.body().unwrap()).unwrap()
}

/// Linked tag "dark" (#111111, no group) on lst_1, already synced once.
fn synced_one(s: &mut ServerGuard) -> (TestDb, i64) {
    let t = TestDb::new();
    let (_, tag) = setup_one(&t);
    mock_create(s, "lst_1");
    mock_ok(s, "POST", "/lists/lst_1/entries");
    pass(&t, s).unwrap();
    (t, tag)
}

#[test]
fn recolour_only_patch_leaves_name_and_group_alone() {
    let mut s = Server::new();
    let (t, tag) = synced_one(&mut s);
    onetagger_db::tags::update_tag(&t.db, tag, None, Some("#222222"), None, None).unwrap();
    let patch = s
        .mock("PATCH", "/lists/lst_1")
        .match_request(|r| patch_body(r) == serde_json::json!({ "color": "#222222" }))
        .with_body("{}")
        .expect(1)
        .create();
    assert_eq!(pass(&t, &s).unwrap().updated, 1);
    patch.assert();
}

#[test]
fn rename_only_patch_leaves_colour_and_group_alone() {
    let mut s = Server::new();
    let (t, tag) = synced_one(&mut s);
    onetagger_db::tags::update_tag(&t.db, tag, Some("darker"), None, None, None).unwrap();
    let patch = s
        .mock("PATCH", "/lists/lst_1")
        .match_request(|r| patch_body(r) == serde_json::json!({ "name": "darker" }))
        .with_body("{}")
        .expect(1)
        .create();
    assert_eq!(pass(&t, &s).unwrap().updated, 1);
    patch.assert();
}

#[test]
fn removed_group_patch_sends_null_group_only() {
    let mut s = Server::new();
    let t = TestDb::new();
    {
        let c = t.db.conn();
        let g = add_group(&c, "mood");
        let tr = add_track(&c, "a");
        set_item(&c, tr, Some("trk_1"));
        let tag = add_tag(&c, "dark", "#111111", Some(g));
        tag_track(&c, tr, tag);
    }
    mock_create(&mut s, "lst_1");
    mock_ok(&mut s, "POST", "/lists/lst_1/entries");
    pass(&t, &s).unwrap();
    t.db.conn().execute("UPDATE tags SET group_id = NULL", []).unwrap();
    let patch = s
        .mock("PATCH", "/lists/lst_1")
        .match_request(|r| {
            patch_body(r) == serde_json::json!({ "ext": { "com.github.rfmussa.flowtag": { "group": null } } })
        })
        .with_body("{}")
        .expect(1)
        .create();
    assert_eq!(pass(&t, &s).unwrap().updated, 1);
    patch.assert();
}

#[test]
fn reused_tag_id_deletes_old_list_and_creates_new_one() {
    let mut s = Server::new();
    let (t, old) = synced_one(&mut s);
    let new = {
        let c = t.db.conn();
        c.execute("DELETE FROM tags WHERE id = ?1", [old]).unwrap();
        let tr = add_track(&c, "b");
        set_item(&c, tr, Some("trk_2"));
        let new = add_tag(&c, "warm", "#333333", None);
        tag_track(&c, tr, new);
        new
    };
    assert_eq!(new, old, "SQLite reuses the max rowid");
    let del = mock_ok(&mut s, "DELETE", "/lists/lst_1").expect(1).create();
    let update = s.mock("PATCH", Matcher::Any).expect(0).create();
    let create = s
        .mock("POST", "/lists")
        .match_body(Matcher::PartialJsonString(r#"{"name":"warm"}"#.into()))
        .with_body(r#"{"id":"lst_2"}"#)
        .expect(1)
        .create();
    mock_ok(&mut s, "POST", "/lists/lst_2/entries");
    let r = pass(&t, &s).unwrap();
    del.assert();
    update.assert();
    create.assert();
    assert_eq!((r.deleted, r.updated, r.created), (1, 0, 1));
}

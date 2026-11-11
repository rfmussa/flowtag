mod common;

use mockito::{Matcher, Server};
use onetagger_tangle::client::Client;
use onetagger_tangle::matcher::{lookup_unmatched, match_from_collection};
use rusqlite::Connection;
use serde_json::{json, Value};

const HOME: &str = "/Users/r";
const HOUR: i64 = 3600;

fn collection() -> Vec<Value> {
    vec![
        json!({"type":"omc","version":"1.0","generated_at":"2026-11-10T00:00:00Z"}),
        json!({"type":"source","id":"src_bc","platform":"bandcamp","name":"Bandcamp"}),
        json!({"type":"source","id":"src_local","platform":"local","name":"Swinsian",
               "ext":{"music.tangle":{"platform":"local_folder"}}}),
        json!({"type":"item","id":"trk_1","kind":"recording","title":"One",
               "copies":[{"source":"src_bc","ref":"bc/1"},
                         {"source":"src_local","ref":"~/Music/Swinsian/A/01 One.mp3","ownership":"file"}]}),
        json!({"type":"item","id":"trk_2","kind":"recording","title":"Two",
               "copies":[{"source":"src_local","ref":"~/Music/Swinsian/A/02 Two.mp3","ownership":"file"}]}),
    ]
}

fn setup() -> (common::TestDb, Vec<i64>) {
    let t = common::TestDb::new();
    let c = t.db.conn();
    c.execute("UPDATE folders SET path = '/Users/r/Music/Swinsian'", []).unwrap();
    let ids = ["A/01 One", "A/02 Two", "A/03 Three"]
        .iter()
        .map(|n| common::add_track(&c, n))
        .collect::<Vec<_>>();
    c.execute("UPDATE tracks SET path = '/Users/r/Music/Swinsian/' || title || '.mp3'", []).unwrap();
    drop(c);
    (t, ids)
}

fn item_of(c: &Connection, track: i64) -> Option<Option<String>> {
    c.query_row("SELECT item_id FROM tangle_items WHERE track_id = ?1", [track], |r| r.get(0)).ok()
}

#[test]
fn maps_folder_to_local_source_and_tracks_to_items() {
    let (t, ids) = setup();
    let c = t.db.conn();
    let s = match_from_collection(&c, &collection(), HOME, 100).unwrap();
    assert_eq!((s.matched, s.total), (2, 3));
    assert_eq!(s.unmatched_paths, vec!["/Users/r/Music/Swinsian/A/03 Three.mp3".to_string()]);
    let src: String = c.query_row("SELECT source_id FROM tangle_sources", [], |r| r.get(0)).unwrap();
    assert_eq!(src, "src_local");
    assert_eq!(item_of(&c, ids[0]), Some(Some("trk_1".into())));
    assert_eq!(item_of(&c, ids[1]), Some(Some("trk_2".into())));
    assert_eq!(item_of(&c, ids[2]), Some(None));
}

#[test]
fn missing_tracks_are_rechecked_after_six_hours_only() {
    let (t, _) = setup();
    let c = t.db.conn();
    match_from_collection(&c, &collection(), HOME, 0).unwrap();
    let mut s = Server::new();
    let m = s
        .mock("GET", "/items")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("source".into(), "src_local".into()),
            Matcher::UrlEncoded("ref".into(), "~/Music/Swinsian/A/03 Three.mp3".into()),
        ]))
        .with_body(r#"{"data":[{"id":"trk_3"}]}"#).expect(1)
        .create();
    let client = Client::new(&s.url(), "tok");
    assert_eq!(lookup_unmatched(&c, &client, HOME, HOUR, 50).unwrap(), 0);
    assert!(!m.matched(), "no request expected yet");
    assert_eq!(lookup_unmatched(&c, &client, HOME, 7 * HOUR, 50).unwrap(), 1);
    assert!(m.matched(), "exactly one request expected");
    // now matched, so never asked again
    assert_eq!(lookup_unmatched(&c, &client, HOME, 20 * HOUR, 50).unwrap(), 0);
    assert!(m.matched(), "exactly one request expected");
}

#[test]
fn lookup_respects_limit() {
    let (t, _) = setup();
    let c = t.db.conn();
    c.execute("INSERT INTO tangle_sources (folder_id, source_id) SELECT id, 'src_local' FROM folders", [])
        .unwrap();
    for i in 0..5 {
        common::add_track(&c, &format!("x{i}"));
    }
    c.execute("UPDATE tracks SET path = '/Users/r/Music/Swinsian/' || title || '.mp3'", []).unwrap();
    let mut s = Server::new();
    let m = s.mock("GET", "/items").match_query(Matcher::Any).with_body(r#"{"data":[]}"#).expect(2).create();
    let client = Client::new(&s.url(), "tok");
    assert_eq!(lookup_unmatched(&c, &client, HOME, 100, 2).unwrap(), 2);
    m.assert();
}

#[test]
fn unmapped_folder_tracks_are_checked_without_request() {
    let (t, ids) = setup();
    let c = t.db.conn();
    let s = Server::new();
    let client = Client::new(&s.url(), "tok");
    assert_eq!(lookup_unmatched(&c, &client, HOME, 100, 50).unwrap(), 3);
    assert_eq!(item_of(&c, ids[0]), Some(None));
}

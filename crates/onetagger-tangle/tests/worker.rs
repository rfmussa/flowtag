//! Worker behaviour seen from outside: commands in, `tangleStatus` / `tangleError` events out.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use mockito::{Matcher, Mock, Server, ServerGuard};
use onetagger_db::Db;
use onetagger_tangle::token::TokenStore;
use onetagger_tangle::worker::{spawn_with, Command, Timing};
use serde_json::Value;

const WAIT: Duration = Duration::from_secs(5);
const ME_OK: &str = r#"{"user":{"id":"u","name":"dj"},"collection_available":true}"#;
const ME_NO_BACKUP: &str = r#"{"user":{"id":"u","name":"dj"},"collection_available":false}"#;

fn fast() -> Timing {
    Timing {
        tick: Duration::from_millis(20),
        debounce: Duration::from_millis(30),
        full_every: Duration::from_secs(300),
        pace: Duration::ZERO,
        rematch_every: Duration::ZERO,
    }
}

/// Always returns "tok" and counts Keychain reads.
#[derive(Clone, Default)]
struct Counting(Arc<AtomicUsize>);

impl Counting {
    fn reads(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

impl TokenStore for Counting {
    fn get(&self) -> Option<String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some("tok".to_string())
    }
    fn set(&self, _: &str) -> anyhow::Result<()> { Ok(()) }
    fn delete(&self) -> anyhow::Result<()> { Ok(()) }
}

struct Started {
    cmd: Sender<Command>,
    rx: Receiver<Value>,
    _dir: tempfile::TempDir,
}

fn start_in(dir: tempfile::TempDir, server: &ServerGuard, tokens: Counting, enabled: bool, timing: Timing) -> Started {
    let (tx, rx) = channel();
    let cmd = spawn_with(
        dir.path().join("w.db"),
        server.url(),
        Box::new(tokens),
        enabled,
        move |ev| { let _ = tx.send(ev); },
        timing,
    )
    .unwrap();
    Started { cmd, rx, _dir: dir }
}

fn start(server: &ServerGuard, enabled: bool) -> Started {
    start_in(tempfile::tempdir().unwrap(), server, Counting::default(), enabled, fast())
}

/// Waits for an event matching `pred`; panics after WAIT.
fn wait_for(rx: &Receiver<Value>, what: &str, pred: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + WAIT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let ev = rx.recv_timeout(left).unwrap_or_else(|_| panic!("no {what} within {WAIT:?}"));
        if pred(&ev) {
            return ev;
        }
    }
}

fn wait_state(rx: &Receiver<Value>, state: &str) -> Value {
    wait_for(rx, state, |ev| ev["action"] == "tangleStatus" && ev["status"]["state"] == state)
}

fn mock_me(s: &mut ServerGuard, body: &str) -> Mock {
    s.mock("GET", "/me").with_body(body).create()
}

fn mock_empty_collection(s: &mut ServerGuard) -> Mock {
    s.mock("GET", "/collection").with_body("{\"type\":\"header\"}\n").create()
}

#[test]
fn sync_off_with_token_still_reports_the_user() {
    let mut s = Server::new();
    let me = s.mock("GET", "/me").with_body(ME_OK).expect(2).create();
    let w = start(&s, false);
    let ev = wait_for(&w.rx, "status with user", |ev| ev["status"]["user"] == "dj");
    assert_eq!(ev["status"]["state"], "disabled");
    // A new token while sync is off is confirmed again.
    w.cmd.send(Command::TokenChanged).unwrap();
    wait_for(&w.rx, "status with user after TokenChanged", |ev| ev["status"]["user"] == "dj");
    me.assert();
}

#[test]
fn backup_required_is_rechecked_without_a_token_change() {
    let mut s = Server::new();
    let no_backup = mock_me(&mut s, ME_NO_BACKUP);
    mock_empty_collection(&mut s);
    let w = start(&s, true);
    wait_state(&w.rx, "backupRequired");
    // The user turns Backup and restore on in Tangle.
    no_backup.remove();
    mock_me(&mut s, ME_OK);
    w.cmd.send(Command::Status).unwrap();
    wait_state(&w.rx, "idle");
}

#[test]
fn enabling_sync_rechecks_backup_required() {
    let mut s = Server::new();
    let no_backup = mock_me(&mut s, ME_NO_BACKUP);
    mock_empty_collection(&mut s);
    let w = start(&s, true);
    wait_state(&w.rx, "backupRequired");
    no_backup.remove();
    mock_me(&mut s, ME_OK);
    w.cmd.send(Command::SetEnabled(true)).unwrap();
    wait_state(&w.rx, "idle");
}

#[test]
fn token_is_read_once_per_start_and_token_change() {
    let mut s = Server::new();
    mock_me(&mut s, ME_OK);
    mock_empty_collection(&mut s);
    let tokens = Counting::default();
    let w = start_in(tempfile::tempdir().unwrap(), &s, tokens.clone(), true, fast());
    wait_state(&w.rx, "idle");
    std::thread::sleep(Duration::from_millis(200)); // ~10 ticks
    assert_eq!(tokens.reads(), 1, "Keychain read on every tick");
    w.cmd.send(Command::TokenChanged).unwrap();
    wait_state(&w.rx, "syncing");
    wait_state(&w.rx, "idle");
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(tokens.reads(), 2);
}

fn wait_error(rx: &Receiver<Value>) -> Value {
    wait_for(rx, "tangleError", |ev| ev["action"] == "tangleError")
}

#[test]
fn failed_preview_reports_an_error_promptly() {
    let mut s = Server::new();
    mock_me(&mut s, ME_OK);
    s.mock("GET", "/collection").with_status(503).create();
    let w = start(&s, false);
    w.cmd.send(Command::Preview).unwrap();
    assert_eq!(wait_error(&w.rx)["code"], "server");
}

#[test]
fn preview_with_rejected_token_reports_invalid_token() {
    let mut s = Server::new();
    s.mock("GET", "/me").with_status(401).create();
    s.mock("GET", "/collection").with_status(401).create();
    let w = start(&s, false);
    w.cmd.send(Command::Preview).unwrap();
    assert_eq!(wait_error(&w.rx)["code"], "invalid_token");
}

/// A folder under the real home directory (the worker maps refs against `$HOME`).
fn add_folder_with_track(db: &Db, name: &str) -> i64 {
    let home = std::env::var("HOME").unwrap();
    let folder = format!("{}/FlowTagWorkerTest-{name}", home.trim_end_matches('/'));
    let c = db.conn();
    c.execute("INSERT INTO folders (path) VALUES (?1)", [&folder]).unwrap();
    let folder_id = c.last_insert_rowid();
    c.execute(
        "INSERT INTO tracks (folder_id, path, format, title, mtime, size) VALUES (?1, ?2, 'mp3', ?3, 0, 0)",
        rusqlite::params![folder_id, format!("{folder}/{name}.mp3"), name],
    )
    .unwrap();
    c.last_insert_rowid()
}

fn item_of(db_path: &Path, track: i64) -> Option<String> {
    Db::open(db_path)
        .unwrap()
        .conn()
        .query_row("SELECT item_id FROM tangle_items WHERE track_id = ?1", [track], |r| r.get(0))
        .unwrap()
}

#[test]
fn folder_added_after_first_match_is_mapped_by_a_later_rematch() {
    let mut s = Server::new();
    mock_me(&mut s, ME_OK);
    let collection = [
        r#"{"type":"header"}"#,
        r#"{"type":"source","id":"src_local","platform":"local"}"#,
        r#"{"type":"item","id":"trk_a","copies":[{"source":"src_local","ref":"~/FlowTagWorkerTest-a/a.mp3"}]}"#,
        r#"{"type":"item","id":"trk_b","copies":[{"source":"src_local","ref":"~/FlowTagWorkerTest-b/b.mp3"}]}"#,
    ]
    .join("\n");
    s.mock("GET", "/collection").with_body(collection).create();
    s.mock("GET", "/items").match_query(Matcher::Any).with_body(r#"{"data":[]}"#).create();
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("w.db");
    let db = Db::open(&db_path).unwrap();
    let a = add_folder_with_track(&db, "a");
    let w = start_in(dir, &s, Counting::default(), true, fast());
    wait_state(&w.rx, "idle");
    assert_eq!(item_of(&db_path, a).as_deref(), Some("trk_a"));

    let b = add_folder_with_track(&db, "b");
    w.cmd.send(Command::SetEnabled(true)).unwrap();
    wait_state(&w.rx, "syncing");
    wait_state(&w.rx, "idle");
    assert_eq!(item_of(&db_path, b).as_deref(), Some("trk_b"));
}

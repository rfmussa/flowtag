//! Background Tangle sync worker: decides when to run passes and reports status.
//! Own thread, own DB connection (like the file flusher). Never logs or emits the token.

use std::path::PathBuf;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{json, Value};

use crate::client::Client;
use crate::error::TangleError;
use crate::matcher::{lookup_unmatched, match_from_collection};
use crate::store;
use crate::sync::run_pass;
use crate::token::TokenStore;
use onetagger_db::Db;

const BACKOFF_BASE_S: u64 = 15;
const BACKOFF_MAX_S: u64 = 1800;
const LOOKUPS_PER_PASS: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncState {
    Disabled,
    NoToken,
    Idle,
    Syncing,
    TokenInvalid,
    BackupRequired,
    Waiting,
    Offline,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TangleStatus {
    pub state: SyncState,
    pub user: Option<String>,
    pub matched: usize,
    pub total: usize,
    pub unmatched_paths: Vec<String>,
    pub pending: usize,
    pub last_synced_at: Option<i64>,
    pub retry_at: Option<i64>,
    pub unlinked: Vec<(i64, String)>,
    pub error: Option<String>,
}

impl TangleStatus {
    fn new(state: SyncState) -> Self {
        TangleStatus {
            state,
            user: None,
            matched: 0,
            total: 0,
            unmatched_paths: Vec::new(),
            pending: 0,
            last_synced_at: None,
            retry_at: None,
            unlinked: Vec::new(),
            error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    SetEnabled(bool),
    TokenChanged,
    ResendTag(i64),
    Status,
    Preview,
}

#[derive(Debug, Clone, Copy)]
pub struct Timing {
    pub tick: Duration,
    pub debounce: Duration,
    pub full_every: Duration,
    pub pace: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            tick: Duration::from_secs(5),
            debounce: Duration::from_secs(3),
            full_every: Duration::from_secs(300),
            pace: Duration::from_millis(500),
        }
    }
}

/// 15 s, doubling per attempt, capped at 30 minutes.
pub fn backoff_s(attempt: u32) -> u64 {
    BACKOFF_BASE_S
        .checked_shl(attempt)
        .filter(|s| *s <= BACKOFF_MAX_S && s >> attempt == BACKOFF_BASE_S)
        .unwrap_or(BACKOFF_MAX_S)
}

/// Run when FlowTag changed and has been quiet for the debounce, or when the last full pass is old.
pub fn should_run(prev: i64, now_fp: i64, stable_for: Duration, since_full: Duration, t: &Timing) -> bool {
    (prev != now_fp && stable_for >= t.debounce) || since_full >= t.full_every
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn spawn(
    db_path: PathBuf,
    base: String,
    tokens: Box<dyn TokenStore>,
    enabled: bool,
    on_event: impl Fn(Value) + Send + 'static,
) -> anyhow::Result<Sender<Command>> {
    spawn_with(db_path, base, tokens, enabled, on_event, Timing::default())
}

pub fn spawn_with(
    db_path: PathBuf,
    base: String,
    tokens: Box<dyn TokenStore>,
    enabled: bool,
    on_event: impl Fn(Value) + Send + 'static,
    timing: Timing,
) -> anyhow::Result<Sender<Command>> {
    // Open eagerly so a broken DB path fails at startup, not silently in the thread.
    let db = Db::open(&db_path)?;
    let home = std::env::var("HOME").unwrap_or_default().trim_end_matches('/').to_string();
    let (tx, rx) = channel::<Command>();
    let mut worker = Worker::new(db, base, tokens, enabled, home, timing, Box::new(on_event));
    std::thread::Builder::new().name("tangle-worker".to_string()).spawn(move || {
        log::info!("Tangle worker started");
        loop {
            match rx.recv_timeout(timing.tick) {
                Ok(cmd) => worker.command(cmd),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            worker.tick();
        }
        log::info!("Tangle worker stopped");
    })?;
    Ok(tx)
}

struct Worker {
    db: Db,
    base: String,
    tokens: Box<dyn TokenStore>,
    enabled: bool,
    home: String,
    timing: Timing,
    emit: Box<dyn Fn(Value) + Send>,
    status: TangleStatus,
    /// TokenInvalid / BackupRequired: no requests until `TokenChanged`.
    paused: bool,
    /// `me()` must be called before the next pass (start, or token changed).
    needs_me: bool,
    /// Run the next pass regardless of the debounce (new token, resend, retry after an error).
    force: bool,
    attempt: u32,
    retry_at: Option<Instant>,
    seen_fp: Option<i64>,
    seen_fp_since: Instant,
    synced_fp: Option<i64>,
    last_full: Option<Instant>,
}

impl Worker {
    fn new(
        db: Db,
        base: String,
        tokens: Box<dyn TokenStore>,
        enabled: bool,
        home: String,
        timing: Timing,
        emit: Box<dyn Fn(Value) + Send>,
    ) -> Self {
        let state = if enabled { SyncState::Idle } else { SyncState::Disabled };
        Worker {
            db,
            base,
            tokens,
            enabled,
            home,
            timing,
            emit,
            status: TangleStatus::new(state),
            paused: false,
            needs_me: true,
            force: true,
            attempt: 0,
            retry_at: None,
            seen_fp: None,
            seen_fp_since: Instant::now(),
            synced_fp: None,
            last_full: None,
        }
    }

    fn emit_status(&self) {
        (self.emit)(json!({ "action": "tangleStatus", "status": self.status }));
    }

    fn set_state(&mut self, state: SyncState) {
        let changed = self.status.state != state;
        self.status.state = state;
        if state != SyncState::Waiting {
            self.status.retry_at = None;
        }
        if changed {
            self.emit_status();
        }
    }

    fn command(&mut self, cmd: Command) {
        match cmd {
            Command::SetEnabled(on) => {
                self.enabled = on;
                self.force = true;
                if !on {
                    self.set_state(SyncState::Disabled);
                }
            }
            Command::TokenChanged => {
                self.paused = false;
                self.needs_me = true;
                self.force = true;
                self.retry_at = None;
                self.attempt = 0;
                self.status.user = None;
                self.status.error = None;
            }
            Command::ResendTag(id) => {
                if let Err(e) = store::resend_tag(&self.db.conn(), id) {
                    log::error!("tangle resend failed: {e}");
                }
                self.force = true;
            }
            Command::Status => self.emit_status(),
            Command::Preview => self.preview(),
        }
    }

    fn token(&mut self) -> Option<String> {
        let token = self.tokens.get();
        if token.is_none() {
            self.set_state(SyncState::NoToken);
        }
        token
    }

    fn preview(&mut self) {
        let Some(token) = self.token() else { return };
        if self.paused {
            self.emit_status();
            return;
        }
        let client = Client::new(&self.base, &token);
        match self.ensure_matched(&client) {
            Ok(()) => {
                let counts = store::preview_counts(&self.db.conn());
                match counts {
                    Ok((tags, entries)) => {
                        (self.emit)(json!({ "action": "tanglePreview", "tags": tags, "entries": entries }));
                    }
                    Err(e) => log::error!("tangle preview failed: {e}"),
                }
                self.refresh_counts();
                self.emit_status();
            }
            Err(e) => self.fail(e),
        }
    }

    /// Bulk-match from the collection export if nothing has been matched yet.
    fn ensure_matched(&mut self, client: &Client) -> Result<(), TangleError> {
        let empty = {
            let c = self.db.conn();
            c.query_row("SELECT NOT EXISTS (SELECT 1 FROM tangle_items)", [], |r| r.get::<_, bool>(0))
                .unwrap_or(false)
        };
        if empty {
            let records = client.collection()?;
            match_from_collection(&self.db.conn(), &records, &self.home, unix_now())
                .map_err(crate::matcher::db_error)?;
        }
        Ok(())
    }

    fn refresh_counts(&mut self) {
        match store::match_counts(&self.db.conn()) {
            Ok((matched, total, paths)) => {
                self.status.matched = matched;
                self.status.total = total;
                self.status.unmatched_paths = paths;
            }
            Err(e) => log::error!("tangle match counts failed: {e}"),
        }
    }

    fn tick(&mut self) {
        if !self.enabled {
            self.set_state(SyncState::Disabled);
            return;
        }
        let Some(token) = self.token() else { return };
        if self.paused || self.retry_at.is_some_and(|t| Instant::now() < t) {
            return;
        }
        let fp = match store::fingerprint(&self.db.conn()) {
            Ok(fp) => fp,
            Err(e) => {
                log::error!("tangle fingerprint failed: {e}");
                return;
            }
        };
        if self.seen_fp != Some(fp) {
            self.seen_fp = Some(fp);
            self.seen_fp_since = Instant::now();
        }
        let prev = self.synced_fp.unwrap_or_else(|| fp.wrapping_add(1));
        let since_full = self.last_full.map_or(Duration::MAX, |t| t.elapsed());
        let due = self.force
            || should_run(prev, fp, self.seen_fp_since.elapsed(), since_full, &self.timing);
        if !due {
            return;
        }
        self.pass(&token, fp);
    }

    fn pass(&mut self, token: &str, fp: i64) {
        self.force = false;
        self.set_state(SyncState::Syncing);
        let client = Client::new(&self.base, token);
        match self.pass_inner(&client) {
            Ok(()) => {
                self.attempt = 0;
                self.retry_at = None;
                self.synced_fp = Some(fp);
                self.last_full = Some(Instant::now());
                self.status.error = None;
                self.status.last_synced_at = Some(unix_now());
                self.status.state = SyncState::Idle;
                self.emit_status();
            }
            Err(e) => self.fail(e),
        }
    }

    fn pass_inner(&mut self, client: &Client) -> Result<(), TangleError> {
        if self.needs_me {
            let me = client.me()?;
            self.status.user = Some(me.user_name);
            if !me.collection_available {
                return Err(TangleError::BackupRequired);
            }
            self.needs_me = false;
        }
        self.ensure_matched(client)?;
        lookup_unmatched(&self.db.conn(), client, &self.home, unix_now(), LOOKUPS_PER_PASS)?;
        let report = run_pass(&self.db, client, self.timing.pace)?;
        self.status.pending = report.pending;
        self.status.unlinked = report.unlinked;
        self.refresh_counts();
        Ok(())
    }

    /// Map a failure to a state, schedule the retry, and report.
    fn fail(&mut self, e: TangleError) {
        let backoff = Duration::from_secs(backoff_s(self.attempt));
        self.status.error = None;
        let state = match &e {
            TangleError::InvalidToken => {
                self.paused = true;
                SyncState::TokenInvalid
            }
            TangleError::BackupRequired => {
                self.paused = true;
                SyncState::BackupRequired
            }
            TangleError::RateLimited { retry_after_s } => {
                let wait = Duration::from_secs(*retry_after_s);
                self.retry_at = Some(Instant::now() + wait);
                self.status.retry_at = Some(unix_now() + *retry_after_s as i64);
                self.force = true;
                SyncState::Waiting
            }
            TangleError::Network(_) | TangleError::Server(_) => {
                self.schedule_retry(backoff);
                SyncState::Offline
            }
            TangleError::Other { code, .. } => {
                self.status.error = Some(code.clone());
                self.schedule_retry(backoff);
                SyncState::Error
            }
            TangleError::ListNotFound | TangleError::ItemNotFound => {
                self.status.error = Some("not_found".into());
                self.schedule_retry(backoff);
                SyncState::Error
            }
        };
        log::warn!("Tangle sync stopped: {e}");
        self.status.state = state;
        self.emit_status();
    }

    fn schedule_retry(&mut self, backoff: Duration) {
        self.attempt = self.attempt.saturating_add(1);
        self.retry_at = Some(Instant::now() + backoff);
        self.force = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use std::sync::mpsc::{channel, Receiver};

    const WAIT: Duration = Duration::from_secs(5);

    fn fast() -> Timing {
        Timing {
            tick: Duration::from_millis(20),
            debounce: Duration::from_millis(30),
            full_every: Duration::from_secs(300),
            pace: Duration::ZERO,
        }
    }

    struct Fake(Option<&'static str>);
    impl TokenStore for Fake {
        fn get(&self) -> Option<String> { self.0.map(str::to_string) }
        fn set(&self, _: &str) -> anyhow::Result<()> { Ok(()) }
        fn delete(&self) -> anyhow::Result<()> { Ok(()) }
    }

    /// Waits for a tangleStatus event with `state`; panics after WAIT.
    fn wait_state(rx: &Receiver<serde_json::Value>, state: &str) -> serde_json::Value {
        let deadline = std::time::Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            let ev = rx.recv_timeout(left).unwrap_or_else(|_| panic!("no {state} status within {WAIT:?}"));
            if ev["action"] == "tangleStatus" && ev["status"]["state"] == state {
                return ev;
            }
        }
    }

    fn start(
        server: &Server,
        token: Option<&'static str>,
    ) -> (Sender<Command>, Receiver<serde_json::Value>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = channel();
        let cmd = spawn_with(
            dir.path().join("w.db"),
            server.url(),
            Box::new(Fake(token)),
            true,
            move |ev| { let _ = tx.send(ev); },
            fast(),
        )
        .unwrap();
        (cmd, rx, dir)
    }

    #[test]
    fn backoff_caps_at_thirty_minutes() {
        assert_eq!(backoff_s(0), 15);
        assert_eq!(backoff_s(1), 30);
        assert_eq!(backoff_s(20), 1800);
        assert_eq!(backoff_s(u32::MAX), 1800);
    }

    #[test]
    fn should_run_debounces_and_forces_every_five_minutes() {
        let t = Timing { tick: Duration::from_secs(5), debounce: Duration::from_secs(3), full_every: Duration::from_secs(300), pace: Duration::ZERO };
        let s = Duration::from_secs;
        assert!(!should_run(1, 1, s(10), s(10), &t), "unchanged: no run");
        assert!(!should_run(1, 2, s(1), s(10), &t), "changed but still being edited");
        assert!(should_run(1, 2, s(3), s(10), &t), "changed and stable");
        assert!(should_run(1, 1, s(0), s(300), &t), "forced full pass");
        assert!(!should_run(1, 1, s(0), s(299), &t));
    }

    #[test]
    fn missing_token_pauses_without_requests() {
        let mut server = Server::new();
        let any = server.mock("GET", mockito::Matcher::Any).expect(0).create();
        let (_cmd, rx, _dir) = start(&server, None);
        wait_state(&rx, "noToken");
        std::thread::sleep(Duration::from_millis(150));
        any.assert();
    }

    #[test]
    fn invalid_token_pauses() {
        let mut server = Server::new();
        let me = server.mock("GET", "/me").with_status(401).expect(1).create();
        let (cmd, rx, _dir) = start(&server, Some("bad"));
        wait_state(&rx, "tokenInvalid");
        std::thread::sleep(Duration::from_millis(200));
        me.assert();
        me.remove();
        // TokenChanged lifts the pause: a fresh /me request is made.
        let again = server.mock("GET", "/me").with_status(401).expect_at_least(1).create();
        cmd.send(Command::TokenChanged).unwrap();
        wait_state(&rx, "tokenInvalid");
        again.assert();
    }
}

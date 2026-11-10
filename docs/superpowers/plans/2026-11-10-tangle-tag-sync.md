# FlowTag → Tangle tag sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** FlowTag tags appear automatically, one-way, as Tangle's own tags.

**Architecture:** A new crate `onetagger-tangle` holds a blocking HTTP client, DB bookkeeping (what was sent), track↔item matching and a sync pass that diffs current tags against what was sent. A worker thread (like the flusher) runs passes when tags change; the UI crate wires it to WebSocket actions and the Settings panel.

**Tech Stack:** Rust (reqwest 0.12 blocking+rustls+json, rusqlite 0.37 via onetagger-db, keyring 3 apple-native, unicode-normalization 0.1, mockito 1 dev-only), Vue 3 + shadcn-vue, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-10-tangle-tag-sync-design.md`

## Global Constraints

- Base URL `https://api.tangle.music/omc/v1`, injectable for tests.
- Every request: `Authorization: Bearer <token>`, `User-Agent: FlowTag/<CARGO_PKG_VERSION>`, `Accept: application/json`. The token is never logged, printed or written to settings.json.
- Keychain: service `flowtag-tangle`, account `pat`.
- Local ref = `"~" + path[len(home):]`, NFC-normalised; `None` outside home.
- Tags are created as `{ name, ordered: false, color }` with no `source`; group goes in `ext["com.github.rfmussa.flowtag"] = { "group": "<name>" }`.
- ≤100 items per entries request; requests paced ≥0.5 s apart.
- Branch on `error.code`, never on message; log `request_id` on unexpected errors.
- NDJSON split on `\n` only (titles can contain U+2028).
- A sent row is recorded only after Tangle confirms the request.
- Never recreate a tag the user deleted in Tangle (`unlinked = 1`) without the user pressing Send again.
- Commits: `<type>: <description>`, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. Renaming a tag onto an existing name merges it (`update_tag` deletes the old tag): the old Tangle tag is deleted and the survivor gains its tracks, with no orphan list. → Task 6 test `merge_deletes_old_list_and_moves_entries`.
2. Tangle answers `unchanged` for entries it already has (e.g. after a crash between request and DB write): treated as success, recorded as sent. → Task 6 test `unchanged_entries_count_as_sent`.
3. A 429 midway through a large first push: confirmed batches are not re-sent, the pass resumes after `Retry-After`. → Task 6 test `rate_limit_midway_keeps_confirmed_batches`.
4. A track whose file moved (new track row, old row gone): the old row's sent entries are removed only if no other track maps to the same item. → Task 4 test `removed_track_keeps_entry_if_another_track_shares_item`.
5. Keychain item missing or unreadable while sync is enabled: status `noToken`, no crash, no requests. → Task 7 test `missing_token_pauses_without_requests`.

---

### Task 1: Crate scaffold + local refs

**Files:**
- Create: `crates/onetagger-tangle/Cargo.toml`, `crates/onetagger-tangle/src/lib.rs`, `crates/onetagger-tangle/src/refs.rs`
- Modify: `Cargo.toml` (workspace `members`: add `"crates/onetagger-tangle"`)

**Interfaces:**
- Produces: `pub fn local_ref(path: &str, home: &str) -> Option<String>` in `refs.rs`; `lib.rs` declares `pub mod refs;` (later tasks add their modules).

Cargo deps: `anyhow 1`, `log 0.4`, `serde 1 (derive)`, `serde_json 1`, `reqwest 0.12 { default-features=false, features=["json","blocking","rustls-tls"] }`, `unicode-normalization 0.1`, `keyring 3 { features=["apple-native"] }`, `onetagger-db = { path = "../onetagger-db" }`. Dev: `mockito 1`, `tempfile 3`.

- [ ] **Step 1: Write failing tests in `refs.rs`**

```rust
#[test] fn home_becomes_tilde() {
    assert_eq!(local_ref("/Users/r/Music/Swinsian/a.mp3", "/Users/r").as_deref(), Some("~/Music/Swinsian/a.mp3"));
}
#[test] fn nfd_path_matches_nfc_ref() {
    // "Xangô" decomposed (o + U+0302) must come out composed
    assert_eq!(local_ref("/Users/r/Xango\u{302}.mp3", "/Users/r").as_deref(), Some("~/Xang\u{f4}.mp3"));
}
#[test] fn outside_home_is_none() { assert_eq!(local_ref("/Volumes/x/a.mp3", "/Users/r"), None); }
#[test] fn home_prefix_needs_separator() { assert_eq!(local_ref("/Users/rteez2/a.mp3", "/Users/rteez"), None); }
```

- [ ] **Step 2: Run** `cargo test -p onetagger-tangle refs` → FAIL (`local_ref` not found).
- [ ] **Step 3: Implement `local_ref`** (strip `home` + `/` prefix, prepend `~/`, `.nfc().collect()`).
- [ ] **Step 4: Run** `cargo test -p onetagger-tangle refs` → 4 passed.
- [ ] **Step 5: Commit** `feat: onetagger-tangle crate with local ref mapping`

---

### Task 2: Migration v9 — sync tables

**Files:**
- Modify: `crates/onetagger-db/src/migrations.rs` (append v9 entry; tests module)

**Interfaces:**
- Produces: tables `tangle_tags`, `tangle_items`, `tangle_sources`, `tangle_sent` exactly as in the spec's "Database (migration v9)" block (copy the SQL verbatim).

- [ ] **Step 1: Write failing test `v9_creates_tangle_tables`**: open `test_db()`, assert each of the four tables exists in `sqlite_master`, and that deleting a `tags` row leaves a `tangle_tags` row with that `tag_id` in place (no FK).
- [ ] **Step 2: Run** `cargo test -p onetagger-db migrations` → FAIL.
- [ ] **Step 3: Append** `M::up(<spec SQL>)` with comment `// v9: Tangle tag sync bookkeeping (no FKs on tag ids on purpose)`.
- [ ] **Step 4: Run** `cargo test -p onetagger-db` → all pass, including `migrations_validate`.
- [ ] **Step 5: Commit** `feat: DB tables for Tangle sync`

---

### Task 3: Tangle HTTP client

**Files:**
- Create: `crates/onetagger-tangle/src/client.rs`, `crates/onetagger-tangle/src/error.rs`
- Test: inline `#[cfg(test)]` with `mockito::Server`

**Interfaces:**
- Produces:
  - `pub enum TangleError { InvalidToken, BackupRequired, RateLimited { retry_after_s: u64 }, ListNotFound, ItemNotFound, Network(String), Server(u16), Other { status: u16, code: String, request_id: Option<String> } }` (impl `std::error::Error`, `Display` without the token).
  - `pub struct Client { base: String, token: String, http: reqwest::blocking::Client }`
  - `Client::new(base: &str, token: &str) -> Client`
  - `fn me(&self) -> Result<Me, TangleError>`; `pub struct Me { pub user_name: String, pub collection_available: bool }` (from `user.name`).
  - `fn collection(&self) -> Result<Vec<serde_json::Value>, TangleError>` (NDJSON, `\n` split).
  - `fn find_item(&self, source: &str, r#ref: &str) -> Result<Option<String>, TangleError>` (`GET /items?source=&ref=`, first `data[].id`).
  - `fn create_list(&self, name: &str, color: Option<&str>, group: Option<&str>) -> Result<String, TangleError>` (returns `id`).
  - `fn update_list(&self, id: &str, name: &str, color: Option<&str>, group: Option<&str>) -> Result<(), TangleError>`
  - `fn delete_list(&self, id: &str) -> Result<(), TangleError>`
  - `fn add_entries(&self, list: &str, items: &[String]) -> Result<(), TangleError>`; `fn remove_entries(&self, list: &str, items: &[String]) -> Result<(), TangleError>` (body `{ "items": [...] }`; both treat any 2xx as success, `unchanged` included).

Error mapping: 401 → `InvalidToken`; 403 code `backup_and_restore_required` → `BackupRequired`; 429 → `RateLimited` from `Retry-After` (default 60); 404 code `list_not_found` → `ListNotFound`, `item_not_found` → `ItemNotFound`; 5xx → `Server`; transport error → `Network`; else `Other`.

- [ ] **Step 1: Write failing tests**
  - `sends_user_agent_and_bearer`: mock `GET /me` matching header `user-agent` regex `^FlowTag/` and `authorization` `Bearer t0k`; `me()` returns `user_name == "rfmussa"`.
  - `maps_error_codes`: 401 → `InvalidToken`; 403 `{"error":{"code":"backup_and_restore_required"}}` → `BackupRequired`; 429 with `Retry-After: 7` → `RateLimited { retry_after_s: 7 }`; 404 `list_not_found` → `ListNotFound`; 502 → `Server(502)`.
  - `collection_keeps_u2028_inside_titles`: body `"{\"type\":\"omc\"}\n{\"type\":\"item\",\"id\":\"trk_1\",\"title\":\"a\u{2028}b\"}\n"` → 2 records, second title `"a\u{2028}b"`.
  - `add_entries_posts_items_body`: mock `POST /lists/lst_1/entries` with JSON body `{"items":["trk_1","trk_2"]}` → `Ok(())`.
  - `create_list_sends_unordered_tag_with_group_ext`: matches body `{"name":"Dark","ordered":false,"color":"#e5484d","ext":{"com.github.rfmussa.flowtag":{"group":"Mood"}}}`, returns `{"id":"lst_9"}` → `"lst_9"`.
- [ ] **Step 2: Run** `cargo test -p onetagger-tangle client` → FAIL.
- [ ] **Step 3: Implement** `client.rs` and `error.rs` per the interfaces above.
- [ ] **Step 4: Run** `cargo test -p onetagger-tangle client` → 5 passed.
- [ ] **Step 5: Commit** `feat: Tangle API client with typed errors`

---

### Task 4: Sync bookkeeping + diff (store)

**Files:**
- Create: `crates/onetagger-tangle/src/store.rs`, `crates/onetagger-tangle/tests/common/mod.rs` (test DB helper: tempdir `Db::open`, raw SQL to insert folders/tracks/tags/track_tags)

**Interfaces:**
- Consumes: `onetagger_db::Db` (`Db::open`, `db.conn()`), Task 2 tables.
- Produces (all `fn(&Connection, …) -> rusqlite::Result<…>`):
  - `pub struct TagRow { pub tag_id: i64, pub name: String, pub color: String, pub group: Option<String> }`
  - `pub struct Linked { pub tag_id: i64, pub list_id: String, pub sent_name: String, pub sent_color: Option<String>, pub sent_group: Option<String>, pub unlinked: bool }`
  - `tags_to_create(c) -> Vec<TagRow>` — FlowTag tags with ≥1 track that has an `item_id`, and no `tangle_tags` row.
  - `tags_to_update(c) -> Vec<(Linked, TagRow)>` — linked, not unlinked, name/color/group differ.
  - `tags_to_delete(c) -> Vec<Linked>` — `tangle_tags` rows whose `tag_id` is not in `tags`.
  - `entries_to_add(c) -> Vec<(String /*list_id*/, Vec<String> /*item ids*/)>` and `entries_to_remove(c) -> Vec<(String, Vec<String>)>` — the spec's set diff, grouped by list, only for linked non-unlinked tags.
  - `link_tag(c, &TagRow, list_id)`, `mark_updated(c, &TagRow)`, `forget_tag(c, tag_id)` (deletes its `tangle_tags` + `tangle_sent` rows), `mark_unlinked(c, tag_id)` (sets `unlinked=1`, deletes sent rows), `resend_tag(c, tag_id)` (= `forget_tag`).
  - `record_sent(c, tag_id, &[String])`, `record_unsent(c, tag_id, &[String])`.
  - `fingerprint(c) -> i64` — `SUM(tag_version)` over tracks combined with a hash of `(id, name, color, group_id)` over tags.

- [ ] **Step 1: Write failing tests** (`tests/store.rs`):
  - `new_tag_with_matched_track_is_created_empty_tag_is_not`.
  - `untag_yields_remove_tag_yields_add` — after `record_sent`, re-tag → add; untag → remove; nothing pending after recording both.
  - `rename_recolor_regroup_yield_update`.
  - `deleted_tag_yields_list_delete`.
  - `tangle_side_entries_never_appear` — a `tangle_sent`-free item in the Tangle list is invisible: diff only reads FlowTag tables + sent.
  - `removed_track_keeps_entry_if_another_track_shares_item` — two tracks → same `item_id`, tagged; delete one track row → no remove; delete both → remove.
  - `unlinked_tag_is_skipped`.
  - `fingerprint_changes_on_tag_untag_rename`.
- [ ] **Step 2: Run** `cargo test -p onetagger-tangle --test store` → FAIL.
- [ ] **Step 3: Implement `store.rs`** (SQL set ops with `EXCEPT`; group name via `LEFT JOIN groups`).
- [ ] **Step 4: Run** `cargo test -p onetagger-tangle --test store` → 8 passed.
- [ ] **Step 5: Commit** `feat: Tangle sync bookkeeping and tag diff`

---

### Task 5: Matching tracks to Tangle items

**Files:**
- Create: `crates/onetagger-tangle/src/matcher.rs`; test `tests/matcher.rs`

**Interfaces:**
- Consumes: `local_ref` (Task 1), `Client::collection`, `Client::find_item` (Task 3).
- Produces:
  - `pub fn match_from_collection(c: &Connection, records: &[Value], home: &str, now: i64) -> rusqlite::Result<MatchSummary>` — fills `tangle_sources` (folder ↔ `local` source whose copy refs start with the folder's `~/…` prefix + `/`) and `tangle_items` for every present track (`item_id` NULL when absent).
  - `pub fn lookup_unmatched(c: &Connection, client: &Client, home: &str, now: i64, limit: usize) -> Result<usize, TangleError>` — tracks with no `tangle_items` row, or NULL `item_id` and `checked_at < now - 6*3600`; at most `limit` (50) per call.
  - `pub struct MatchSummary { pub matched: usize, pub total: usize, pub unmatched_paths: Vec<String> }` (≤50 paths kept).

- [ ] **Step 1: Write failing tests**: fixture collection (header, one `local` source with refs under `~/Music/Swinsian/`, a Bandcamp source, two items) + DB with folder `/Users/r/Music/Swinsian` and 3 tracks (2 present in fixture, 1 not):
  - `maps_folder_to_local_source_and_tracks_to_items` → `matched 2, total 3`, one unmatched path.
  - `missing_tracks_are_rechecked_after_six_hours_only` (mockito `GET /items` hit count 0 at +1 h, 1 at +7 h).
  - `lookup_respects_limit`.
- [ ] **Step 2: Run** `cargo test -p onetagger-tangle --test matcher` → FAIL.
- [ ] **Step 3: Implement `matcher.rs`**.
- [ ] **Step 4: Run** → 3 passed.
- [ ] **Step 5: Commit** `feat: match FlowTag tracks to Tangle items`

---

### Task 6: Sync pass

**Files:**
- Create: `crates/onetagger-tangle/src/sync.rs`; test `tests/sync.rs`

**Interfaces:**
- Consumes: Tasks 3–5.
- Produces:
  - `pub fn run_pass(db: &Db, client: &Client, pace: Duration) -> Result<PassReport, TangleError>` — order: delete lists → update lists → create lists → remove entries → add entries; chunks of 100; `std::thread::sleep(pace)` between requests; DB updated after each confirmed request (lock `db.conn()` only around DB work, never across HTTP).
  - `ListNotFound` on update/delete/entries → `mark_unlinked` (delete: `forget_tag`), continue. `ItemNotFound` → clear that track's `item_id`, continue. Any other error → return it (already-confirmed work stays recorded).
  - `pub struct PassReport { pub created: usize, pub updated: usize, pub deleted: usize, pub added: usize, pub removed: usize, pub pending: usize, pub unlinked: Vec<(i64, String)> }`

- [ ] **Step 1: Write failing tests** (mockito, `pace = 0`):
  - `first_pass_creates_tags_and_adds_entries`; `second_identical_pass_sends_nothing` (mock hit counts unchanged).
  - `untag_removes_entry`; `rename_patches_list`; `delete_removes_list`.
  - `batches_of_100` — 250 tagged tracks → 3 entry POSTs of 100/100/50.
  - `list_not_found_marks_unlinked_and_is_not_recreated` (no `POST /lists` on the next pass).
  - `merge_deletes_old_list_and_moves_entries` (use `onetagger_db::tags::update_tag` rename onto an existing name).
  - `unchanged_entries_count_as_sent` (200 `{"unchanged":["trk_1"]}` → recorded).
  - `rate_limit_midway_keeps_confirmed_batches` (2nd of 3 POSTs → 429; returns `RateLimited`; next pass posts only the remaining 150).
- [ ] **Step 2: Run** `cargo test -p onetagger-tangle --test sync` → FAIL.
- [ ] **Step 3: Implement `sync.rs`**.
- [ ] **Step 4: Run** `cargo test -p onetagger-tangle` → all pass.
- [ ] **Step 5: Commit** `feat: Tangle sync pass`

---

### Task 7: Token store + worker

**Files:**
- Create: `crates/onetagger-tangle/src/token.rs`, `crates/onetagger-tangle/src/worker.rs`

**Interfaces:**
- Produces:
  - `pub trait TokenStore: Send { fn get(&self) -> Option<String>; fn set(&self, token: &str) -> anyhow::Result<()>; fn delete(&self) -> anyhow::Result<()>; }`; `pub struct KeychainToken` (keyring `Entry::new("flowtag-tangle", "pat")`).
  - `#[serde(rename_all = "camelCase")] pub struct TangleStatus { pub state: SyncState, pub user: Option<String>, pub matched: usize, pub total: usize, pub unmatched_paths: Vec<String>, pub pending: usize, pub last_synced_at: Option<i64>, pub retry_at: Option<i64>, pub unlinked: Vec<(i64, String)>, pub error: Option<String> }`
  - `#[serde(rename_all = "camelCase")] pub enum SyncState { Disabled, NoToken, Idle, Syncing, TokenInvalid, BackupRequired, Waiting, Offline, Error }`
  - `pub enum Command { SetEnabled(bool), TokenChanged, ResendTag(i64), Status }`
  - `pub fn spawn(db_path: PathBuf, base: String, tokens: Box<dyn TokenStore>, enabled: bool, on_status: impl Fn(&TangleStatus) + Send + 'static) -> anyhow::Result<std::sync::mpsc::Sender<Command>>` — own `Db::open`, thread name `tangle-worker`; first enabled pass with no `tangle_items` rows runs `match_from_collection`; each pass also runs `lookup_unmatched(…, 50)`.
  - `pub fn backoff_s(attempt: u32) -> u64` — 15 s doubling, capped at 1800.
  - `pub fn should_run(prev: i64, now_fp: i64, stable_for: Duration, since_full: Duration) -> bool` — changed and stable ≥3 s, or ≥5 min since the last full pass.

- [ ] **Step 1: Write failing tests**: `backoff_caps_at_thirty_minutes` (`backoff_s(0)==15`, `backoff_s(20)==1800`); `should_run_debounces_and_forces_every_five_minutes`; `missing_token_pauses_without_requests` (fake `TokenStore` returning `None`, mockito with zero expected hits, `on_status` receives `NoToken`); `invalid_token_pauses` (401 → `TokenInvalid`, no further requests until `TokenChanged`).
- [ ] **Step 2: Run** `cargo test -p onetagger-tangle worker` → FAIL.
- [ ] **Step 3: Implement `token.rs` + `worker.rs`** (loop: `recv_timeout(5s)` on the command channel; map `TangleError` to `SyncState` per the spec's error table; `RateLimited` → `Waiting` with `retry_at`).
- [ ] **Step 4: Run** `cargo test -p onetagger-tangle` → all pass.
- [ ] **Step 5: Commit** `feat: Tangle sync worker and Keychain token store`

---

### Task 8: Backend wiring (UI crate)

**Files:**
- Modify: `crates/onetagger-ui/Cargo.toml` (dep `onetagger-tangle`), `crates/onetagger-ui/src/lib.rs` (spawn after the flusher, store `tangle: Sender<Command>` in `WsState`), `crates/onetagger-ui/src/socket.rs` (actions + handler)

**Interfaces:**
- Consumes: `worker::spawn`, `Command`, `TangleStatus`, `KeychainToken`, `Client::me`.
- Produces WebSocket actions (camelCase, in `Action`): `tangleConnect { token: String }` (verify with `me()` first; on success `KeychainToken.set`, send `TokenChanged`; reply `{action:"tangleConnected", user}` or `{action:"tangleError", code}`), `tangleDisconnect` (`delete`, `TokenChanged`), `tangleStatus`, `tangleSetEnabled { enabled: bool }`, `tanglePreview` (reply `{action:"tanglePreview", tags, entries}` from `tags_to_create` + `entries_to_add` counts), `tangleResendTag { tagId: i64 }`. Status broadcast as `{ "action": "tangleStatus", "status": <TangleStatus> }` on the existing `events` channel.
- Startup `enabled` = `Settings::load()?.ui["flowtag"]["tangleSync"].as_bool().unwrap_or(false)`.

- [ ] **Step 1: Write failing test** in `socket.rs` tests: `tangle_actions_deserialize` — each JSON (`{"action":"tangleConnect","token":"x"}`, `{"action":"tangleSetEnabled","enabled":true}`, `{"action":"tangleResendTag","tagId":3}`, …) parses to its `Action` variant.
- [ ] **Step 2: Run** `cargo test -p onetagger-ui tangle` → FAIL.
- [ ] **Step 3: Implement** the variants, handlers and spawn.
- [ ] **Step 4: Run** `cargo test -p onetagger-ui && cargo build` → pass.
- [ ] **Step 5: Commit** `feat: wire Tangle sync into the app server`

---

### Task 9: Settings panel + delete confirm

**Files:**
- Create: `client/src/lib/tangle.ts`, `client/src/lib/tangle.test.ts`, `client/src/components/flowtag/TangleSettings.vue`
- Modify: `client/src/scripts/settings.ts` (`FlowTagSettings.tangleSync: boolean = false`, read in `fromJson`), `client/src/scripts/onetagger.ts` (`tangleStatus: Ref<TangleStatus | null>`; `incomingEvent` cases `tangleStatus`, `tangleConnected`, `tangleError`, `tanglePreview`), `client/src/components/Settings.vue` (new `ft-label section` "Tangle" before "Advanced", rendering `TangleSettings`), `client/src/components/flowtag/TagManagerRow.vue:154` (append `" Also deletes it from Tangle."` when `settings.flowtag.tangleSync`)

**Interfaces:**
- Produces: `export function statusLine(s: TangleStatus, now: number): string` in `lib/tangle.ts`, with the spec's copy: `Synced · 2 min ago`, `Sending 340 changes…`, `6 tracks aren't in Tangle`, `Token expired. Paste a new one`, `Tangle needs Backup and restore on`, `Waiting for Tangle until 14:05`, `Offline, retrying`, `1 tag was deleted in Tangle`.

- [ ] **Step 1: Write failing tests** `tangle.test.ts`: one `it` per state above with exact strings (2 min from `lastSyncedAt = now - 120`; `14:05` from a `retryAt` built with local time).
- [ ] **Step 2: Run** `cd client && pnpm test` → FAIL.
- [ ] **Step 3: Implement** `statusLine` and `TangleSettings.vue`: password `Input` + Save (`tangleConnect`), "Connected as {user} · {matched} of {total} tracks found in Tangle", Disconnect (with the revoke reminder), `Switch` "Sync tags to Tangle" — first enable sends `tanglePreview`, shows `confirmDialog` "This creates {tags} tags in Tangle and adds {entries} tracks to them." and only then saves `tangleSync` + sends `tangleSetEnabled`; status line with a hover list of unmatched paths; per unlinked tag a **Send again** button (`tangleResendTag`).
- [ ] **Step 4: Run** `pnpm test && pnpm run build` → pass.
- [ ] **Step 5: Commit** `feat: Tangle section in Settings`

---

### Task 10: Install + live check (with the user)

- [ ] **Step 1:** `cd client && pnpm run build`, then run the scratchpad `reinstall.sh` (touch lib.rs, release build, bundle, reinstall `/Applications/FlowTag.app`).
- [ ] **Step 2:** User pastes the token in Settings → Tangle. Expected: "Connected as rfmussa · 7,672 of 7,678 tracks found in Tangle".
- [ ] **Step 3:** User turns the switch on. Expected: confirmation with real counts; after OK, the 6 tags appear in Tangle with their colours.
- [ ] **Step 4:** Throwaway tag "FlowTag test" on 2–3 tracks → appears in Tangle within ~10 s; rename → renamed; untag one → removed; delete → gone in Tangle.
- [ ] **Step 5:** `cargo test --workspace && (cd client && pnpm test)` green; push the branch.

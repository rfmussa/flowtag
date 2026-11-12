# Staging & continuity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A git-like stage → commit flow for tagging, with durable done state, view restore, a commit journal (today, chart, trail) and progress scoped to what the user staged.

**Architecture:** New DB module `staging.rs` (stage/skip/commit/revert/journal on migration v11's tables) behind new WebSocket actions in the UI crate; the Vue client gains a tri-state quick filter, a pinned Stage scope, a commit sheet, a Journal dialog, a welcome-back card and view restore via settings.json. Smart-playlist filters stay client-evaluated.

**Tech Stack:** Rust (rusqlite via onetagger-db), axum WebSocket actions, Vue 3 + shadcn-vue, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-12-staging-continuity-design.md`

## Global Constraints

- Keys: `S` stage/unstage toggle, `X` skip toggle, `⌘Enter` (metaKey or ctrlKey + Enter) commit sheet; `B`/`J`/`K`/`T`/`F`/`D`/`E`/`M` keep today's meanings; new codes join `reservedFlowtagCodes()` (client/src/scripts/flowkeys.ts:52).
- Done status, stage membership, commit messages and edit_log are local only: no file writes, no `needs_flush` changes, nothing to Tangle.
- edit_log records QuickTag user edits only — never scan, flusher, comment export or Tangle sync.
- Status derivation (one definition everywhere): done (done_at NOT NULL) → staged → in progress (tagIds.length > 0) → untagged; "not done" = all but done.
- Migration v11 is appended after v10, `IF NOT EXISTS` style, SQL per the spec's Data section verbatim.
- WebSocket actions are camelCase variants of `Action` (crates/onetagger-ui/src/socket.rs:33); replies/events broadcast like existing tagflow messages.
- Coding style: new files focused (<~400 lines); immutable update patterns in TS; no console.log.
- Commits: `<type>: <description>` + exactly: Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>

## Review Focus

1. Re-staging an already-done track and committing again must not corrupt state (stays done, done_commit moves to the new commit) → Task 2 test `recommit_of_done_track_updates_commit_id`.
2. Reverting a commit after the user has since edited those tracks' tags must only clear done/restage, never touch tags → Task 2 test `revert_leaves_tags_untouched`.
3. With quick filter "Not done", committing the selected track removes it from view; the cursor must land on a neighbour, not vanish (Lightroom's classic bug) → Task 6 test `commit_keeps_cursor_on_neighbour`.
4. Old saved smart filters (`untaggedOnly`, tag rules) must evaluate unchanged next to the new status rule → Task 5 test `legacy_filters_still_pass`.
5. Active time with a single edit today, or gaps spanning midnight, must be 0 / counted only within today → Task 3 test `active_time_gap_cap_and_day_bounds`.

---

### Task 1: Migration v11 + stage/skip core

**Files:**
- Create: `crates/onetagger-db/src/staging.rs`; declare `pub mod staging;` in `crates/onetagger-db/src/lib.rs`
- Modify: `crates/onetagger-db/src/migrations.rs` (append v11)
- Test: inline `#[cfg(test)]` in staging.rs using `crate::testutil::{test_db, insert_track}`

**Interfaces:**
- Produces: tables `staged`, `commits`, `edit_log`, columns `tracks.done_at`, `tracks.done_commit` (spec SQL verbatim); and in `staging.rs`:
  - `pub struct StageRow { pub track_id: i64, pub skip: bool, pub ready: bool }` (ready = skip || has tags)
  - `stage_tracks(db: &Db, ids: &[i64]) -> Result<usize, Error>` (INSERT OR IGNORE, staged_at = now; done tracks may be staged too)
  - `unstage_tracks(db: &Db, ids: &[i64]) -> Result<usize, Error>`
  - `toggle_skip(db: &Db, track_id: i64) -> Result<bool, Error>` (auto-stages when missing; returns new skip)
  - `stage_list(db: &Db) -> Result<Vec<StageRow>, Error>` (order: staged_at, then tracks.path)
  - `stage_counts(db: &Db) -> Result<(usize, usize), Error>` (ready, total)
  - `library_done_count(db: &Db) -> Result<usize, Error>`
  - `folder_done_counts(db: &Db) -> Result<Vec<(i64, usize, usize)>, Error>` (folder_id, done, total; missing = 0 excluded from total)

- [ ] **Step 1: Write failing tests** — `v11_creates_staging_tables` (tables + columns exist on `test_db()`); `stage_unstage_idempotent` (staging twice = 1 row; unstage twice ok); `toggle_skip_autostages` (X on unstaged track → staged, skip=1, ready; toggle again → skip=0, not ready); `ready_means_skip_or_tags` (tag a staged track via `tags::add_tag_to_track` → ready); `counts_and_folder_counts` (set done_at by SQL on 2 of 3 tracks → library 2; folder (2,3)).
- [ ] **Step 2: Run** `cargo test -p onetagger-db staging` → FAIL.
- [ ] **Step 3: Implement** v11 + staging.rs functions above (log nothing yet — edit_log writes land in Task 3).
- [ ] **Step 4: Run** `cargo test -p onetagger-db` → all pass incl. `migrations_validate`.
- [ ] **Step 5: Commit** `feat: staging tables and stage/skip core`

---

### Task 2: Commit and revert

**Files:**
- Modify: `crates/onetagger-db/src/staging.rs` (+ tests)

**Interfaces:**
- Consumes: Task 1 tables/functions.
- Produces:
  - `pub struct CommitRow { pub id: i64, pub created_at: i64, pub message: String, pub track_count: i64 }`
  - `commit_stage(db: &Db, track_ids: &[i64], message: &str) -> Result<CommitRow, Error>` — one transaction: each id gets done_at = now, done_commit = new commit id, row removed from `staged`; commits row with track_count = ids.len(); ids not currently staged are ignored (not an error).
  - `revert_commit(db: &Db, commit_id: i64) -> Result<usize, Error>` — clears done_at/done_commit where done_commit = id, re-stages those tracks (skip preserved? no: skip=0, staged_at=now), deletes the commits row; returns track count.
  - `list_commits(db: &Db, limit: usize) -> Result<Vec<CommitRow>, Error>` (newest first)

- [ ] **Step 1: Write failing tests** — `commit_marks_done_and_unstages` (3 staged, commit 2 ticked → 2 done with same commit id, 1 still staged, commits.track_count = 2); `commit_ignores_unstaged_ids`; `recommit_of_done_track_updates_commit_id` (stage a done track, commit → done_at refreshed, done_commit = new id); `revert_restages_and_clears_done` (+ commits row gone); `revert_leaves_tags_untouched` (add a tag after commit, revert → tag still there).
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4:** `cargo test -p onetagger-db` → pass. **Step 5: Commit** `feat: stage commit and revert`

---

### Task 3: Edit log + journal queries

**Files:**
- Modify: `crates/onetagger-db/src/staging.rs` (log + journal), `crates/onetagger-db/src/tags.rs` (log calls in `add_tag_to_track`, `add_tag_to_tracks`, `remove_tag_from_track(s)`, `update_tag` rename arm, `delete_tag`, `get_or_create_tag` creation)
- Test: staging.rs tests + one in tags.rs

**Interfaces:**
- Consumes: Tasks 1–2.
- Produces:
  - `log_event(c: &Connection, track_id: Option<i64>, kind: &str, data: &serde_json::Value) -> rusqlite::Result<()>` with kinds exactly: `tag_added, tag_removed, skip_set, skip_cleared, staged, unstaged, committed, commit_reverted, tag_created, tag_renamed, tag_deleted` (stage/skip/commit ops log themselves from Tasks 1–2 code paths; tags.rs logs the tag kinds with `{"tagId", "tagName"}`).
  - `pub struct TodayStats { pub committed: usize, pub tags_created: usize, pub active_secs: i64 }`
  - `today_stats(db: &Db, day_start: i64, now: i64) -> Result<TodayStats, Error>` — active_secs = Σ min(gap, 300) over consecutive edit_log entries in [day_start, now], 0 for <2 entries.
  - `commit_days(db: &Db, limit: usize) -> Result<Vec<(String, usize)>, Error>` — (YYYY-MM-DD local via SQLite `date(created_at,'unixepoch','localtime')`, tracks committed), only days with commits, newest first, `limit` days.
  - `trail(db: &Db, before_id: Option<i64>, limit: usize) -> Result<Vec<TrailRow>, Error>`; `pub struct TrailRow { pub id: i64, pub at: i64, pub track_id: Option<i64>, pub track_title: Option<String>, pub kind: String, pub data: String }`
- Scan/flusher/Tangle never call `log_event` (they don't call these tags.rs fns; verify `scan.rs` doesn't — if it does, gate with a `log: bool` param per the spec).

- [ ] **Step 1: Write failing tests** — `tag_mutations_write_log` (add/remove/rename/delete → kinds in order); `stage_ops_write_log`; `commit_and_revert_write_log`; `active_time_gap_cap_and_day_bounds` (entries at t, t+100, t+10000, one yesterday → active = 100 + 300); `commit_days_buckets_only_commit_days`; `trail_pages_newest_first_with_titles`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4:** `cargo test -p onetagger-db` → pass. **Step 5: Commit** `feat: edit log and journal queries`

---

### Task 4: WebSocket actions + track rows carry status

**Files:**
- Modify: `crates/onetagger-ui/src/socket.rs` (Action variants + dispatch), `crates/onetagger-ui/src/tagflow.rs` (handlers; extend the track row payload of `tagTracksLoad` with `doneAt: Option<i64>`, `staged: bool`, `skip: bool`)
- Test: `tangle_actions_deserialize`-style test in socket.rs tests

**Interfaces:**
- Consumes: staging.rs API (Tasks 1–3).
- Produces actions (camelCase): `stageTracks { trackIds: Vec<i64> }`, `unstageTracks { trackIds: Vec<i64> }`, `stageSkip { trackId: i64 }`, `stageCommit { trackIds: Vec<i64>, message: String }`, `stageRevert { commitId: i64 }`, `journalGet { beforeId: Option<i64> }` → replies `{action:"stageState", ready, total, libraryDone, folderCounts:[[folderId,done,total]]}` (broadcast after every mutating action and after tag edits), `{action:"stageCommitted", commit:{id,createdAt,message,trackCount}}`, `{action:"journal", today:{…}, days:[[date,count]], trail:[…], commits:[…]}`.
- Every tag/stage mutation already triggers the client to reload rows via existing events; reuse that path (whatever `tagUpdate`/`trackAddTag` broadcast today) rather than inventing new row events.

- [ ] **Step 1: Write failing test** `staging_actions_deserialize` (each new JSON → variant).
- [ ] **Step 2: Run** `cargo test -p onetagger-ui` → FAIL. **Step 3: Implement** variants + handlers + row payload fields. **Step 4:** `cargo test -p onetagger-ui && cargo build` → pass. **Step 5: Commit** `feat: staging actions over the websocket`

---

### Task 5: Client model — status, tri-state filter, Status smart rule

**Files:**
- Modify: `client/src/scripts/tagstore.ts` (FlowTrack gains `doneAt?: number|null, staged?: boolean, skip?: boolean`; `untaggedOnly: Ref<boolean>` → `quickFilter: Ref<'all'|'notdone'|'untagged'>`; stage scope; `stageState` message handling)
- Create: `client/src/lib/status.ts` + `client/src/lib/status.test.ts`
- Modify: `client/src/components/flowtag/SmartPlaylistEditor.vue` (Status rule UI), smart-filter evaluation in tagstore (`passes`, tagstore.ts:279-295)

**Interfaces:**
- Produces: `export type TrackStatus = 'done'|'staged'|'inProgress'|'untagged'`; `statusOf(t: {doneAt?: number|null, staged?: boolean, tagIds: number[]}): TrackStatus`; `matchesStatus(t, rule: {status: TrackStatus|'notDone', negate?: boolean}): boolean`; smart filter JSON gains optional `status?: { is?: string, isNot?: string }` alongside existing keys; quick filter predicate: notdone = status !== 'done', untagged = status === 'untagged'.
- Consumes: Task 4 row fields and `stageState` events.

- [ ] **Step 1: Write failing Vitest** — `status_precedence` (done beats staged beats inProgress); `quickfilter_notdone_hides_done_only`; `smart_rule_status_is_and_is_not`; `legacy_filters_still_pass` (an old filter JSON with `untaggedOnly:true` + tag includes evaluates as before on tracks that now carry doneAt/staged).
- [ ] **Step 2:** `cd client && pnpm test` → FAIL. **Step 3: Implement.** **Step 4:** `pnpm test && pnpm run build` → pass. **Step 5: Commit** `feat: track status model and Status smart rule`

---

### Task 6: Keys + Stage scope + toolbar

**Files:**
- Modify: `client/src/scripts/flowkeys.ts` (S, X, ⌘Enter; reserved codes), `client/src/views/QuickTag.vue` (tri-state toggle All·Not done·Untagged; "Stage next 50" button; counter chip → "N done"), `client/src/components/flowtag/PlaylistSidebar.vue` (pinned Stage entry: name, ready/total, thin bar; opens stage scope), `client/src/components/flowtag/TrackList.vue` + `TrackRow` (status chip: ✓ done · staged · skip · for later), `client/src/components/flowtag/FolderManager.vue` (done/total per folder row)
- Test: extend `client/src/lib/status.test.ts` with cursor logic if extracted; otherwise rely on `selectNextUntagged`-style store fn + its test

**Interfaces:**
- Consumes: Tasks 4–5 (`stageTracks`/`unstageTracks`/`stageSkip` sends; `stageState` in store refs `stageReady`, `stageTotal`, `libraryDone`, `folderCounts`).
- Produces: store fns `toggleStageSelection()`, `stageNext50()` (first 50 not-done & not-staged in current visible order), `selectNeighbourAfterRemoval(trackId)` reused on commit; stage scope = `scope: Ref<'all'|'stage'|number>` (number = playlist id, today's behaviour preserved).
- Context menus: track/selection rows gain "Stage" / "Unstage"; sidebar playlist rows and FolderManager folder rows gain "Stage its not-done tracks" — first 200 not-done not-staged of that scope, count shown in the label.
- `D` inside stage scope: next staged row with ready === false.

- [ ] **Step 1: Write failing Vitest** for the pure parts — `stage_next_50_picks_not_done_not_staged_in_view_order`; `commit_keeps_cursor_on_neighbour` (remove current id from visible list → selection moves to the next row, wrapping like `selectNextUntagged`).
- [ ] **Step 2:** `pnpm test` → FAIL. **Step 3: Implement** keys, scope, toolbar, chips, folder counts. **Step 4:** `pnpm test && pnpm run build` → pass. **Step 5: Commit** `feat: stage keys, scope and progress toolbar`

---

### Task 7: Commit sheet + quit prompt

**Files:**
- Create: `client/src/components/flowtag/CommitSheet.vue` (shadcn Dialog; staged list with checkboxes — ready pre-ticked, not-ready unticked and labelled; message Input, placeholder "What were you doing?"; counts; Commit disabled when 0 ticked; Enter confirms, Esc cancels)
- Modify: `client/src/scripts/flowkeys.ts` (⌘Enter opens it — added in Task 6's reserved list, wired here), `client/src/scripts/onetagger.ts` (exit flow at the `case 'exit'` handler, onetagger.ts:352: when stage ready > 0 → confirmDialog "Commit N ready staged tracks first?" with Commit & quit / Quit anyway paths), mount in `client/src/App.vue`

**Interfaces:**
- Consumes: `stageCommit` action, `stageCommitted` event, store stage list (Task 6), `confirmDialog` (client/src/lib/dialogs.ts).
- Produces: `openCommitSheet()` exported from the store (used by key, Stage header button, quit flow).

- [ ] **Step 1: Write failing Vitest** for tick logic in `client/src/lib/commit.ts`: `ticked_defaults_ready_only`, `commit_payload_is_ticked_ids_and_trimmed_message`.
- [ ] **Step 2:** `pnpm test` → FAIL. **Step 3: Implement** sheet + quit prompt. **Step 4:** `pnpm test && pnpm run build` → pass. **Step 5: Commit** `feat: commit sheet and quit prompt`

---

### Task 8: Journal dialog

**Files:**
- Create: `client/src/components/flowtag/JournalDialog.vue` (Today stats; bar chart = plain divs, one bar per day from `days`, heights relative to max; "current folder N% done" text from folderCounts + current scope; trail list grouped by day, newest first, per-line Undo, Revert on commit lines; "Load more" via `journalGet{beforeId}`)
- Create: `client/src/lib/journal.ts` + test (formatting: active time "1h 12m", trail line text per kind, inverse op mapping)
- Modify: `client/src/views/QuickTag.vue` ("N done" chip opens it), `client/src/scripts/onetagger.ts` (`journal` event → ref)

**Interfaces:**
- Consumes: `journalGet`/`journal`, `stageRevert`; inverse ops reuse existing actions: tag_added → `trackRemoveTag`, tag_removed → `trackAddTag`, skip_set → `stageSkip`, staged → `unstageTracks`, committed → `stageRevert` (labelled Revert); tag_created/renamed/deleted lines show no Undo.
- Produces: `trailLine(row): {text: string, undo?: {action: string, data: any}}` in journal.ts.

- [ ] **Step 1: Write failing Vitest** — `active_time_formats`, `trail_lines_and_inverses` (one case per kind incl. no-undo kinds), `chart_scales_to_max_day`.
- [ ] **Step 2:** `pnpm test` → FAIL. **Step 3: Implement.** **Step 4:** `pnpm test && pnpm run build` → pass. **Step 5: Commit** `feat: journal with today, chart and trail`

---

### Task 9: View restore + welcome back

**Files:**
- Modify: `client/src/scripts/settings.ts` (`FlowTagSettings.viewState?: ViewState`, `FlowTagSettings.welcomeSeen?: string`), `client/src/scripts/tagstore.ts` (apply/save viewState debounced via `$1t.saveSettings(false)`), `client/src/views/QuickTag.vue` (restore on mount; card)
- Create: `client/src/components/flowtag/WelcomeCard.vue`, `client/src/lib/welcome.ts` + test

**Interfaces:**
- Produces: `interface ViewState { scope: 'all'|'stage'|number; filterText: string; quickFilter: 'all'|'notdone'|'untagged'; sort: <existing sort type>; selectedTrackId: number|null; firstVisibleIndex: number }`; `welcomeText(lastCommit: CommitRow|null, stageLeft: number, now: number): string|null` (null = nothing to show); card shown when text ≠ null and `welcomeSeen !== key(lastCommitId, stageLeft)`; Continue applies viewState, Dismiss writes welcomeSeen.
- Missing selected track / deleted playlist → fall back to 'all' scope silently.

- [ ] **Step 1: Write failing Vitest** — `welcome_text_quotes_commit_message_and_counts`, `welcome_hidden_when_seen_key_matches`, `viewstate_fallback_on_missing_playlist`.
- [ ] **Step 2:** `pnpm test` → FAIL. **Step 3: Implement.** **Step 4:** `pnpm test && pnpm run build` → pass. **Step 5: Commit** `feat: view restore and welcome back`

---

### Task 10: Install + live check (with the user)

- [ ] **Step 1:** `cargo test --workspace && (cd client && pnpm test && pnpm run build)` all green; then the reinstall script (touch onetagger-ui lib.rs, release build, bundle, reinstall /Applications/FlowTag.app).
- [ ] **Step 2:** DB backup taken before first launch (same pattern as the Tangle check); confirm v11 applied, 7,678 tracks intact.
- [ ] **Step 3:** With the user: stage a few tracks (S + Stage next 50), tag some, X one, ⌘Enter → commit with a message; check the chip, stage bar and folder counts move; quit prompt appears with ready tracks.
- [ ] **Step 4:** Relaunch → welcome card quotes the message, view restored; Journal shows today, the bar and the trail; Undo a tag line; Revert the commit → tracks re-staged, done cleared.
- [ ] **Step 5:** Check Tangle: only tag edits synced, no stage/done/commit traces; push the branch.

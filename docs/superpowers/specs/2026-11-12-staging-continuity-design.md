# FlowTag staging & continuity — design

Date: 2026-11-12 · Status: approved in conversation, awaiting spec review
Research behind it: `reports/Multi session tagging continuity.md` (+ notes in `research_notes/`)
Mockups: canvas "FlowTag Suggestions Mockups", boards 5–7 (adjusted per conversation: staging replaces batches)

## Goal

Tagging ~7,700 tracks happens a few dozen at a time over many evenings. FlowTag
must carry a thread across sessions: where you were, what is truly finished,
what you set aside, and visible progress against a denominator you can actually
reach tonight — without streaks, quotas or guilt mechanics.

## The model (user's own words: "I select some tracks to a staging environment and tag those")

git-like, three layers:

| Layer | Meaning | How |
|---|---|---|
| **Staged** | tonight's (or this week's) working set, picked by the user | `S` on selection, "Stage next 50" button, stage survives restarts |
| **Ready** | a staged track with ≥1 tag, or skipped with `X` ("nothing to add") | computed, shown as ✓ in the Stage view |
| **Done** | sealed by a **commit**: review sheet → untick stragglers → optional message → confirm | `tracks.done_at` set; leaves every queue |

Decisions fixed with the user:
- Tagging keys are UNCHANGED (`Enter` adds a tag and stays; existing `D`/`J`/`K` as today). The "am I finished?" decision moves to commit time, in bulk.
- Unticked / unfinished staged tracks stay staged across restarts — the visible "for later" pile, never silently lost.
- The commit log is the continuity thread: Welcome back quotes the last commit message; the trail is the log; the chart is one bar per day committed.
- Done status, stage membership, commit messages and the edit log are LOCAL ONLY: never written to audio files, never synced to Tangle.
- No streaks, no daily quotas, no reminders; the library-wide number stays secondary and framed as done ("465 done"), never "55 / 7678".

## Keys (new ones join `reservedFlowtagCodes()`; conflict warning in Manage tags as today)

| Key | Action |
|---|---|
| `S` | stage / unstage the selected track(s) — toggle; works on multi-selection (`B` keeps the sidebar) |
| `X` | toggle "nothing to add" skip on the current track; auto-stages it if not staged |
| `⌘Enter` | open the commit sheet (anywhere in QuickTag); in the sheet `Enter` confirms, `Esc` cancels |
| `D` | as today (next untagged); in the Stage view: next staged track that isn't ready |
| `⌘Z` | undo, as today (in-memory stack), including stage/skip toggles |

## Data (migration v11, appended; follow v9/v10 conventions — IF NOT EXISTS, comments)

```sql
CREATE TABLE IF NOT EXISTS staged (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    staged_at INTEGER NOT NULL,
    skip INTEGER NOT NULL DEFAULT 0            -- X: "listened, nothing to add"
);
CREATE TABLE IF NOT EXISTS commits (
    id INTEGER PRIMARY KEY,
    created_at INTEGER NOT NULL,
    message TEXT NOT NULL DEFAULT '',
    track_count INTEGER NOT NULL
);
ALTER TABLE tracks ADD COLUMN done_at INTEGER;          -- NULL = not done
ALTER TABLE tracks ADD COLUMN done_commit INTEGER;      -- commits.id; no FK on purpose (revert is code's job)
CREATE TABLE IF NOT EXISTS edit_log (
    id INTEGER PRIMARY KEY,
    at INTEGER NOT NULL,
    track_id INTEGER,                                   -- NULL for tag/group-level events
    kind TEXT NOT NULL,   -- tag_added | tag_removed | skip_set | skip_cleared | staged | unstaged
                          -- | committed | commit_reverted | tag_created | tag_renamed | tag_deleted
    data TEXT NOT NULL DEFAULT '{}'                     -- JSON: tagId, tagName, commitId, count…
);
CREATE INDEX IF NOT EXISTS idx_edit_log_at ON edit_log(at);
```

Status of a track (single derivation, used by UI, filters and smart playlists):
**done** (done_at NOT NULL) → else **staged** (row in `staged`) → else
**in progress** (has tags) → else **untagged**. "Not done" = everything but done.

The edit log is written at the DB mutation choke points (`tags.rs` add/remove/
update/delete, stage/skip/commit ops) — QuickTag edits only; scans, the
flusher, comment export and Tangle sync never log. The log powers the trail,
Today, the chart and active time; it is not an undo journal (undo stays the
in-memory stack; commits get explicit Revert).

## Behaviours

### Staging
- `S` toggles the selection in/out of `staged` (staged_at = now).
- **Stage next 50**: toolbar button, enabled in any view; stages the first 50
  not-done, not-staged tracks of the current view order.
- Context menu on a track/selection: "Stage" / "Unstage"; on a sidebar playlist
  or folder: "Stage its not-done tracks" (capped at 200 with a count in the label).
- The **Stage** appears pinned at the top of the playlist sidebar with
  "14 / 32" and a bar (ready/total). Opening it scopes the list to staged
  tracks (order: staged_at, then path). Row status chips: ✓ ready ·
  "staged" · "skip".

### Commit
- `⌘Enter` (or the Stage header's Commit button) opens the sheet: the staged
  list with ready ones ticked (not-ready unticked and marked), a one-line
  message field (optional, placeholder "What were you doing?"), counts, and
  Commit/Cancel.
- Commit: one transaction — ticked tracks get done_at/done_commit, leave
  `staged`; a `commits` row and `committed` log entries are written. Unticked
  stay staged. Toast: "Committed 27 tracks".
- Empty stage or nothing ticked → sheet says so, Commit disabled.
- **Quit prompt**: the existing exit confirm gains "Commit 27 ready staged
  tracks first?" (Commit & quit / Quit / Cancel) when ready > 0.
- **Revert** (trail line, commits only): clears done_at/done_commit for that
  commit's tracks, re-stages them, logs `commit_reverted`. No cap; any commit
  can be reverted.

### Welcome back (view restore)
- `ui.flowtag.viewState` in settings.json (saved via the existing debounced
  `saveSettings(false)`): route, scope (stage | playlist id | all), filter
  text, quick-filter mode, sort, selected track id, first visible index.
  Restored on launch; a missing track/playlist falls back gracefully.
- A dismissible card on first open of QuickTag when there is a previous
  commit or a non-empty stage: "Tuesday 23:41 · committed 27 · 'check the
  Strausz vocal cuts' · Stage: 9 left" + Continue (applies viewState scope)
  / Dismiss. Never shown twice for the same state.

### Filters & smart playlists
- Toolbar quick filter becomes three-state: **All · Not done · Untagged**
  (replaces the two-state untaggedOnly; persisted in viewState).
- Smart playlist rule **Status** `is / is not` one of `done, staged,
  in progress, untagged, not done`, combinable with existing rules, evaluated
  wherever existing rules are (backend playlists filter + client mirror, same
  as current fields).

### Progress surfaces
- **Stage bar** (sidebar + Stage view headline): ready / total staged;
  flips to "N left" when past half, per the research.
- **Folder %**: sidebar folders show done/total with a thin bar.
- **Library**: toolbar chip becomes "465 done" (count of done); clicking it
  opens the **Journal** dialog.
- **Journal** dialog (new, shadcn Dialog): Today (committed today, tags
  created, active time = sum of gaps between consecutive edit-log entries
  today, each gap capped at 5 min), the chart (one bar per day-with-commits,
  last 10 such days; current folder % as text), and the trail (edit log,
  newest first, grouped by day; per-line Undo = apply the inverse op now;
  commit lines get Revert). Toolbar "tagged/total" counter is replaced by
  this chip.

## Out of scope (deliberate)
Streaks, points, quotas, reminder nags; batch machinery (packing/splitting —
staging replaced it); "done before tag group existed" rule (later); player
position restore; syncing done/stage/log anywhere; sets-of-50 auto progress
outside the stage; editing commit messages after the fact.

## Non-interference
- `needs_flush` / flusher behaviour untouched (stage/done/log are not tag data).
- Tangle sync untouched: the diff reads track_tags only; done/staged never leave the app.
- Scan/missing handling: staged rows cascade with tracks; done_at survives
  file moves (same row).

## Testing (TDD throughout)
- DB: migration v11 on a v10 db; stage/unstage/skip idempotence; commit
  transaction (ticked → done + unstaged, unticked stay); revert restores
  stage and clears done; status derivation precedence; edit-log entries per
  op; active-time computation (gap cap).
- Client (Vitest): status/quick-filter predicate incl. "not done"; smart-rule
  Status mirror; welcome-card formatter (times, counts, message); chart
  bucketing (days with commits only); commit-sheet tick logic (ready
  pre-ticked).
- Live check together: stage a handful of Demo/real tracks, tag, X one,
  commit with a message, relaunch → welcome card + restored view, revert the
  commit, check Tangle saw only the tag edits.

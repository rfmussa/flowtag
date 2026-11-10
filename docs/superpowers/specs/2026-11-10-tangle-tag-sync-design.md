# FlowTag → Tangle tag sync — design

Date: 2026-11-10 · Status: approved in conversation, awaiting spec review

## Goal

FlowTag's tags appear in Tangle as Tangle's own tags (unordered lists with no
source), so the user can browse their FlowTag tagging inside Tangle's apps next
to the rest of their collection. Personal tool: one user, one Mac, one Tangle
account (`rfmussa`).

## Decisions (from the user)

| Question | Decision |
|---|---|
| Destination | Tangle's own tags only. Not Rekordbox My Tags (no platform writes). |
| Purpose | Browse in Tangle. |
| Trigger | Automatic, in the background, a few seconds after tagging. |
| Direction | One-way, FlowTag → Tangle. |
| Edits made inside Tangle | Left alone: FlowTag only sends its own changes and never reverts Tangle-side edits. |
| Auth | The user's personal access token (`tng_pat_…`) from the macOS Keychain. No OAuth app. |

## Verified facts this design relies on (live account, 2026-11-10)

- `collection_available: true`; Core 1.0 + Access 0.2.
- Tangle's local-folder copy `ref` is the **home-relative absolute path**
  (`~/Music/Swinsian/…/01 Xangô.mp3`), not the folder-relative form MIX prefers.
  7,672 of 7,678 present FlowTag tracks match by `"~" + path[len(home):]` after
  NFC normalisation; misses are the Demo Tracks folder (not in Tangle) and 4
  `.mp4` files.
- FlowTag folders map 1:1 to Tangle `local` sources (`complete`, `downloads`,
  `Music Downloads`, `Swinsian`).
- No name collisions: FlowTag has 6 tags; Tangle has one own tag (`juke`).
- Cloudflare in front of `api.tangle.music` rejects Python-urllib's default
  User-Agent (error 1010); every request must send a UA (`FlowTag/<version>`).

## Approach: compare against what was sent

A table records what FlowTag has already sent (tag × Tangle item). A worker
periodically diffs current FlowTag tags against it:

- in `track_tags`, not in sent → **add** the entry in Tangle;
- in sent, not in `track_tags` → **remove** the entry in Tangle.

Tangle-side edits are in neither side of the diff, so they are never touched.
No hooks in the tagging code; self-healing after crashes or time offline.

Rejected: an outbox written by every tag-mutating path (easy to miss a path,
silent drift); bulk `/import` (20/hour, meant for one-off moves).

## Architecture

### New crate `crates/onetagger-tangle`

Keeps networking out of `onetagger-db`. Depends on `onetagger-db`, `reqwest`
(blocking, rustls, json — already in the workspace), `serde`/`serde_json`,
`unicode-normalization`, `keyring` (apple-native) for the Keychain.

Modules (each small, independently testable):

- `refs.rs` — `local_ref(path, home) -> Option<String>`: `~/…` form, NFC;
  `None` outside home.
- `client.rs` — thin blocking HTTP client over `https://api.tangle.music/omc/v1`
  (base URL injectable for tests). Endpoints: `GET /me`, `GET /collection`
  (NDJSON, split on `\n` only), `GET /items?source=&ref=`, `POST /lists`,
  `PATCH /lists/{id}`, `DELETE /lists/{id}`, `POST`/`DELETE /lists/{id}/entries`.
  Always sends `Authorization: Bearer`, `User-Agent: FlowTag/<version>`,
  `Accept: application/json`. Maps errors by `error.code` (never by message)
  into a typed `TangleError` (`InvalidToken`, `BackupRequired`, `RateLimited
  { retry_after }`, `ListNotFound`, `ItemNotFound`, `Network`, `Server`, `Other
  { code, request_id }`). Never logs the token.
- `store.rs` — DB access for the new tables + the diff queries.
- `matcher.rs` — first-sync collection download → track ↔ item and folder ↔
  source mapping; single lookups for unmatched tracks later.
- `sync.rs` — one sync pass: tag lifecycle (create/rename/recolour/delete),
  then entry adds/removes in batches of ≤100 (`entries_per_request`).
- `token.rs` — Keychain read/write/delete, service `flowtag-tangle`, account
  `pat`. The token never touches `settings.json` or logs.

### Database (migration v9)

No foreign keys on the sync tables on purpose: after a FlowTag tag or track is
deleted, the worker still needs the Tangle ids to undo what it sent.

```sql
CREATE TABLE tangle_tags (
    tag_id INTEGER PRIMARY KEY,          -- FlowTag tags.id (may no longer exist)
    list_id TEXT NOT NULL UNIQUE,        -- Tangle lst_…
    sent_name TEXT NOT NULL,
    sent_color TEXT,
    sent_group TEXT,
    unlinked INTEGER NOT NULL DEFAULT 0  -- deleted in Tangle: stop syncing it
);
CREATE TABLE tangle_items (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    item_id TEXT,                        -- NULL: not found in Tangle (yet)
    checked_at INTEGER NOT NULL
);
CREATE TABLE tangle_sources (
    folder_id INTEGER PRIMARY KEY REFERENCES folders(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL              -- Tangle src_… (platform local)
);
CREATE TABLE tangle_sent (
    tag_id INTEGER NOT NULL,
    item_id TEXT NOT NULL,
    PRIMARY KEY (tag_id, item_id)
);
```

Diff, conceptually (only for linked, non-unlinked tags; only tracks with an
`item_id`):

- adds = `{(tag, item)}` from `track_tags ⋈ tangle_items` − `tangle_sent`
- removes = `tangle_sent` − that same set

Two FlowTag tracks can map to one Tangle item (same song in two folders); the
set semantics dedupe that naturally.

### Matching

- **First sync**: `GET /collection` once (costs 60 reads). For each `local`
  source collect copy refs; map each FlowTag folder to the source whose refs
  start with the folder's `~/…` prefix → `tangle_sources`. Fill
  `tangle_items` for every present track (`item_id` NULL when absent).
- **Later**: tracks without a `tangle_items` row, or with NULL `item_id` and
  `checked_at` older than 6 h, are looked up singly with
  `GET /items?source=&ref=`, at most 50 per pass.
- `ItemNotFound` on a write → clear that `item_id` (re-lookup later); its sent
  rows are dropped.

### Tags

- New FlowTag tag (no `tangle_tags` row) with at least one matched track →
  `POST /lists { name, ordered: false, color, ext }`. Empty tags are not created.
- Name/colour/group differ from `sent_*` → `PATCH /lists/{id}`.
- `tangle_tags` row whose `tag_id` no longer exists in `tags` → `DELETE
  /lists/{id}`, then drop its rows. FlowTag's delete-tag confirm adds "Also
  deletes it from Tangle."
- Group → `ext["com.github.rfmussa.flowtag"] = { "group": "<name>" }` (Tangle
  apps can't create folders yet). Colour → `color` (`#rrggbb`).
- `ListNotFound` (the user deleted it in Tangle) → set `unlinked = 1`, drop its
  sent rows, never recreate automatically. Settings offers **Send again**
  (clears the row so the next pass recreates it).

### Worker

A thread started with the UI server, like the file flusher. It runs only when
sync is enabled and a token exists.

- Every 5 s it computes a cheap fingerprint: `SUM(tag_version)` over tracks +
  a hash of `(id, name, color, group_id)` over tags. If it changed and has been
  stable for 3 s → run a pass. Otherwise a full pass every 5 min (also picks up
  tracks Tangle has scanned since).
- A sent row is written only after Tangle confirms the request (writes are
  atomic per request), so a failure simply reappears in the next diff.
  Adds/removes are idempotent on Tangle's side (`unchanged`).
- Pushes a `tangleStatus` event to the UI after every pass and state change.

### Settings and UI

`settings.json` gains `flowtag.tangleSync: bool` (default false). The token
lives only in the Keychain.

Settings → new **Tangle** section:

- Token: password field + Save → stored in the Keychain, verified with `/me`
  → "Connected as rfmussa · 7,672 of 7,678 tracks found in Tangle".
  Disconnect deletes the Keychain item and reminds the user to revoke the token
  in Tangle's dashboard.
- Switch **Sync tags to Tangle**. First enable shows a one-time confirmation
  with real counts ("This creates 6 tags in Tangle and adds N tracks to them").
- One status line: `Synced · 2 min ago` · `Sending 340 changes…` ·
  `6 tracks aren't in Tangle` (hover lists them) · or an error state below.

WebSocket actions: `tangleConnect { token }`, `tangleDisconnect`,
`tangleStatus`, `tangleSetEnabled { enabled }`, `tanglePreview` (counts for
the first-enable confirm), `tangleResendTag { tagId }`.

## Error handling

| Condition | Behaviour | Status line |
|---|---|---|
| 401 `invalid_token` | Pause sync | Token expired. Paste a new one |
| 403 `backup_and_restore_required` | Pause sync | Tangle needs Backup and restore on |
| 429 (incl. hourly limits: 10 list deletes, 2,000 removals) | Sleep `Retry-After`, resume | Waiting for Tangle until HH:MM |
| Network / 5xx | Exponential backoff (cap 30 min), like the flusher | Offline, retrying |
| `list_not_found` | Mark tag unlinked (above) | 1 tag was deleted in Tangle · Send again |
| `item_not_found` | Clear item id, re-lookup later | adds to "not in Tangle" |
| Any other code | Log code + `request_id`, back off | Tangle error (code) |

Writes stay under 120/min by pacing requests ≥0.5 s apart.

## Testing

TDD; tests written first.

- **Unit**: `local_ref` (home → `~`, NFC vs NFD `Xangô`, outside home →
  `None`); diff queries on an in-memory DB with real migrations (tag, untag,
  rename, recolour, delete, two tracks → one item, Tangle-side edits never
  appear); batching (250 → 100/100/50); error-code mapping.
- **Client vs fake server** (`mockito`, dev-dependency): UA + Bearer on every
  request; `Retry-After` honoured; 401 → `InvalidToken`; NDJSON with U+2028 in
  a title parses.
- **Sync pass end to end vs fake server**: first run creates tags and adds
  entries; untag → remove; rename → PATCH; delete → DELETE; second identical
  pass sends nothing; `list_not_found` → unlinked, not recreated.
- **Migration**: v9 applies on a v8 database.
- **Frontend**: Vitest for the status-line formatter.
- **Live check, together with the user**: connect → counts shown; enable →
  confirm real first push; throwaway tag "FlowTag test" on 2–3 tracks →
  appears in Tangle; rename, untag one, delete → each reflected in Tangle.

## Out of scope

Playlists and smart playlists; two-way sync; Rekordbox My Tags; listens;
OAuth; DJ-module analysis (LUFS, sections); syncing the Demo Tracks folder.

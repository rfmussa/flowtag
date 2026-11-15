# FlowTag Architecture

The map of the system and the decisions that shaped it. What the product *is* lives in
[PRODUCT.md](PRODUCT.md); the visual system lives in [DESIGN.md](DESIGN.md). This file and its
siblings record how the code is put together and why, so a session picking up work cold does not
have to re-derive it:

- [crates/onetagger-player/ARCHITECTURE.md](crates/onetagger-player/ARCHITECTURE.md) — the deck (audio playback)
- [crates/onetagger-ui/ARCHITECTURE.md](crates/onetagger-ui/ARCHITECTURE.md) — the server (axum, websocket, flusher)
- [crates/onetagger/ARCHITECTURE.md](crates/onetagger/ARCHITECTURE.md) — the desktop shell (wry/tao)
- [client/ARCHITECTURE.md](client/ARCHITECTURE.md) — the Vue client

## Process model

FlowTag is a local web app wearing a native coat. One binary, three layers in one process:

```
FlowTag.app  →  onetagger binary (crates/onetagger: wry/tao shell, main thread)
                 └─ server thread: axum on http://127.0.0.1:36913 (crates/onetagger-ui)
                     ├─ serves the embedded Vue client (client/dist, baked in at compile time)
                     ├─ /ws websocket: every UI action and event
                     └─ owns the deck (crates/onetagger-player), the library DB, the flusher
                 └─ WKWebView window loading http://127.0.0.1:36913
```

The port is `onetagger_shared::PORT = 36913` and is a compile-time constant — a second instance
cannot bind it, and the dev client (Vite on :8080) talks to whatever instance holds it.

CLI modes (`crates/onetagger/src/main.rs`): plain launch opens the shell; `-S/--server` runs the
server headless; `--expose` binds 0.0.0.0; `--browser` opens the system browser; `-p/--path`
pre-opens a music folder.

## The One-Deck Invariant

There is exactly one audio pipeline in the entire application: one shared player (the "deck",
a process-wide singleton in `crates/onetagger-ui/src/socket.rs`) holding one long-lived rodio
sink (`crates/onetagger-player/src/lib.rs`). Overlapping or leftover audio is structurally
impossible, not just unlikely. Both halves of that sentence were earned through real bugs —
the crate docs above tell the war stories. Any future feature that wants a second audible
source (a headphone cue deck) must be built as a deliberate, named second deck.

## Repo layout

| Path | What it is |
|---|---|
| `client/` | Vue 3 + Vite + Tailwind v4 UI; built to `client/dist`, embedded into the server crate |
| `crates/onetagger` | Desktop shell binary: wry/tao window, mini window, media keys, drag-drop |
| `crates/onetagger-ui` | Axum server, websocket protocol, preload slot, flusher, embedded client |
| `crates/onetagger-player` | The deck: rodio playback, per-format sources, seek, device rebind |
| `crates/onetagger-db` | SQLite library DB, tag/track schema, flush + analysis logic |
| `crates/onetagger-tag` | Per-format tag IO (ID3/FLAC/MP4/AIFF), incl. `set_rating`/`get_rating` |
| `crates/onetagger-tangle` | One-way tag sync worker to Tangle |
| `crates/onetagger-shared` | Settings, logging, `PORT`, version info |
| `crates/onetagger-{autotag,platforms,tagger,renamer,playlist,cli}` | Upstream OneTagger machinery (Autotagger etc.), largely untouched |

Upstream heritage: this is a fork of Marekkon5/onetagger. FlowTag replaced Quick Tag; the other
upstream views still exist on the legacy look. When something seems oddly general for a
one-person tool, it is usually upstream inheritance.

## Build and ship (macOS)

```
cd client && pnpm build            # client → client/dist
touch crates/onetagger-ui/src/lib.rs   # force re-embed of dist (include_dir has no rerun tracking)
cargo build --release
cargo bundle --release             # → target/release/bundle/osx/FlowTag.app
codesign --force --deep -s A35B6D1D6D00A02541D2EA6CD9F8FB6639D9BEB8 target/release/bundle/osx/FlowTag.app
```

The identity is the local "Apple Development: Renato Mussa" cert; signing with the same identity
keeps keychain approvals persistent across installs. Install = quit the running app, replace
`/Applications/FlowTag.app`, relaunch.

## Verification

- `cargo check --all-targets` — the whole workspace.
- `cargo test` — Rust suites. Caveat: `onetagger-platforms` contains live-network scrape tests
  (`beatsource::test_beatsource` fails when the site changes); exclude it for a clean local run.
- `cd client && pnpm test` — vitest, 14 files / ~127 tests, all in `client/src/lib`. The pattern:
  `client/src/lib` holds pure, testable logic (player end-of-track, commit flow, keybinds, quit
  flow); `client/src/scripts` holds the stateful app wiring. Put new logic in `lib/` with a test.

## Branch and PR state (2026-11-14)

Stacked PRs on `feat/flowtag`, merge in order:

1. **#1** `fix/client-singleton-leak` — the get1t() singleton leak (425k leaked app objects/hour
   class; see client doc).
2. **#2** `feat/themes` — eight themes + settings picker.
3. **#3** `fix/single-deck` — the One-Deck Invariant (singleton deck + single reused sink).

Parked, deliberately unmerged: `claude/laughing-diffie-4e010e` holds two complete shell
migrations. **Tauri 2** was rejected: Tauri inherits wry's macOS 26 WKWebView resize breakage
(tauri-apps/tauri#14288, #14843) and would need the same frame-pinning workaround the hand-rolled
shell already has. **Electron** works end to end and was blocked only by the client leak — fixed
in #1, so it is viable again if ever wanted; that branch also carries a `--media` server mode
(souvlaki Now Playing from the headless server process) that is cherry-pickable for any shell.

## Star ratings (quality axis)

`tracks.rating` (v12: 1–5, NULL = unrated, column CHECK) plus `rating_pending`. Set through
`tracksSetRating` → `onetagger-db::ratings::set_rating` (validates, dirties via the usual
`needs_flush`/`tag_version` bump, sets `rating_pending`) → `tracksRatingChanged` broadcast. The
flusher calls `onetagger-tag`'s `set_rating` (POPM for MP3/AIFF — Rekordbox reads it natively)
**only while `rating_pending`**, so a tag-only flush never rewrites or wipes a rating FlowTag
didn't set; the flag clears with `needs_flush` under the same `tag_version` guard. Scans import
the file's rating like bpm/key/year, except while one is pending. v12 resets stored stats once
so the first scan after upgrading backfills every file's rating. Client: ⇧1–⇧5 (see the client
doc's keyboard map), a toggleable stars column, sort by rating, a "≥ n ★" header filter and
smart-playlist rule (`lib/rating.ts`).

## Agreed but unbuilt

- **Energy chips** (set-building axis): a tag group `E1 Chill · E2 Warm-up · E3 Groove ·
  E4 Driving · E5 Peak`, cool→hot dot ramp, exactly one per track. Stays chips on purpose —
  energy has no native field in DJ software and already exports via the comment hashtags.

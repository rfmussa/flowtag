# onetagger — the desktop shell

The thinnest layer: a hand-rolled wry/tao window around the local server. `main.rs` parses the
CLI, spawns `onetagger_ui::start_all` on a background thread, and runs the webview event loop on
the main thread pointing at `http://127.0.0.1:36913/`. In `--server` mode the webview never
starts. The shell deliberately knows nothing about tagging — it owns exactly the things a web
page cannot: windows, OS media keys, folder drops, and quitting.

## Windows

The main window is 1280×720 (min 800×450), dark theme, with the app-colored background
`(17,17,18)` so live-resize exposure stays ink instead of white. The **mini window** is a
frameless, always-on-top 400×178 window on `http://127.0.0.1:36913/#/mini` — a second, ordinary
client of the same server (which is why server state, not window state, is the source of truth).
It is created and torn down by `CustomWindowEvent::ToggleMini` on the event loop.

## The IPC protocol

The page talks to the shell through wry's raw `window.ipc.postMessage(string)`; the shell talks
to the page by evaluating `window.onWebviewEvent({...})`. The whole vocabulary:

| Direction | Message | Meaning |
|---|---|---|
| page → shell | `devtools` | Open the webview inspector |
| page → shell | `exit` | The page approves quitting; the event loop exits |
| page → shell | `toggleMini` | Open/close the mini window |
| page → shell (mini) | `closeMini` | Close the mini window |
| page → shell (mini) | `dragMini` | Start a window drag (frameless mini has no titlebar) |
| shell → page | `{action: "exit"}` | Close was requested; page checks for unsaved work, answers `exit` |
| shell → page | `{action: "browse", path}` | A folder was dropped on the window |

The exit handshake matters: `WindowEvent::CloseRequested` never closes directly — the shell asks
the page first, and the page replies with ipc `exit` only after its quit flow (commit-ready
staged tracks, tagging-in-progress confirm) is satisfied. Drag-drop accepts exactly one dropped
*folder* (→ `browse`); single *files* are passed through to the page's own HTML5 handlers.

## Media keys and Now Playing

souvlaki (`MediaControls`) must live on the main thread beside the event loop, so the shell owns
it. Inbound: `onetagger_ui::media::register_media_sink` relays the client's player state (sent
over the websocket) to the main thread as a user event, which updates `MPNowPlayingInfoCenter` —
title, artist, position, and cover art fetched from `/thumb`. Outbound: OS commands
(play/pause/next/prev/seek) land in `handle_media_event`, which calls
`onetagger_ui::media::media_key` — a broadcast to every client; the main window executes.

## The macOS 26 frame-pinning workaround (keep it)

macOS 26 stopped honoring the WKWebView autoresizing mask for window-attached webviews, and
wry's `set_bounds` is a no-op for them: shrink the window and the page stays laid out for the old
size, clipped, with dead space. Commit `afa4906` handles `WindowEvent::Resized` and pins the
webview's frame to its superview's bounds (main and mini windows both). This must stay for as
long as the shell is wry-based — it is not legacy cruft, it is the resize path.

## Shell history: Tauri rejected, Electron parked

Both alternatives were fully built on branch `claude/laughing-diffie-4e010e` (unmerged, kept as
reference):

- **Tauri 2** (complete, rejected): Tauri sits on the same wry, so it inherits the exact macOS 26
  resize bug (tauri-apps/tauri#14288, #14843 — open upstream, unfixed through wry 0.57) and would
  need the same frame-pinning workaround. Migrating to it bought nothing but a new config surface.
  Bonus trap recorded there: pairing stable Tauri requires pinning `tauri-runtime 2.9.2` +
  `wry 0.53.4` in the lockfile or it does not compile.
- **Electron** (complete, viable): no WKWebView, so the resize bug class cannot exist. It was
  blocked by a catastrophic client memory leak under Chromium — which turned out to be the
  `get1t()` singleton bug (see [client/ARCHITECTURE.md](../../client/ARCHITECTURE.md)), fixed in
  PR #1, so Electron is open again if the wry papercuts ever outweigh its ~250MB cost. That
  branch also carries a `--media` server mode (souvlaki in the headless server process, pumped
  CFRunLoop, ppid watchdog) that any non-Rust shell would need — cherry-pickable.

*Revisit if:* a macOS update breaks the next thing in wry — the decision record above says what
each exit costs before anyone re-litigates it from scratch.

## Build

`cargo bundle --release` produces `target/release/bundle/osx/FlowTag.app` from the
`[package.metadata.bundle]` section in this crate's Cargo.toml (identifier `com.rteez.flowtag`);
sign with the Apple Development identity noted in the root [ARCHITECTURE.md](../../ARCHITECTURE.md).
Remember the client must be rebuilt and `onetagger-ui` touched first, or the .app ships a stale UI.

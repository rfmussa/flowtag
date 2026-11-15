# onetagger-ui — the server

The middle of the sandwich: an axum HTTP + websocket server on `127.0.0.1:36913`
(`onetagger_shared::PORT`) that serves the embedded client and owns everything stateful — the
deck, the library DB, the preload slot, the background flusher. The shell is just a window onto
it; `--server` mode runs it alone.

## HTTP surface (src/lib.rs)

`start_all` builds the DB, spawns the flusher and the Tangle worker, then serves:

| Route | What |
|---|---|
| `/` and `/{*path}` | The embedded client. `client/dist` is baked in via `include_dir!` at compile time — **gotcha:** `include_dir` has no rebuild tracking, so after `pnpm build` you must `touch crates/onetagger-ui/src/lib.rs` or cargo serves the stale embed. |
| `/ws` | The websocket. Everything the UI does goes through it. |
| `/thumb`, `/audio` | Album art and raw audio by `?path=`. `/thumb` is also what macOS Now Playing fetches for cover art. |
| `/spotify` | OAuth callback for the upstream Autotagger. |

## The websocket and SocketContext (src/socket.rs)

Each connection gets a `SocketContext` — its sender, per-connection volume/gain, Spotify state —
and messages are handled **sequentially** per connection (`handle_message(...).await` inline in
the read loop; there is no per-message spawn, so actions from one window can never reorder).
Two channels fan out: `context.send` to this window, `context.broadcast` (a tokio broadcast of
strings) to every window — used for state echoes that must keep all windows honest: tag changes,
`nowPlayingChanged`, `mediaKey`, and since the deck became shared, `playerEnded`.

## DECISION: the deck is a process-wide singleton

`SocketContext` used to own an `AudioPlayer` per connection. The webview's websocket can die and
be replaced without a page load, and the dying connection's player kept its sink sounding while
the fresh one played the next track on top — two tracks audible at once. Any second client (the
mini window, a browser tab on :36913) could do the same by existing. The deck is now one static
`OnceLock<Mutex<AudioPlayer>>` (`deck()` in socket.rs): every connection drives the same player,
so there is no second pipeline to leak. Its `Ended` events ride the shared broadcast; whichever
connection first touches the deck spawns the single forwarder thread, which outlives connections.

Tradeoffs, accepted knowingly: independent per-client playback is gone (relevant only to
`--server --expose` multi-device use, where clients now steal the deck from each other); all
windows must follow **mirror-don't-drive** — only the main window acts on `playerEnded`
(guarded client-side), every other window renders state and sends intents; and the deck is a
single point of failure behind a mutex (lock holds are milliseconds; `Seek` is the longest).
A pleasant side effect: playback survives websocket reconnects.

*Revisit if:* a cue/preview deck is wanted (build a second *named* deck, don't regress to
per-connection players), or `--server --expose` becomes a real multi-user mode.

## The preload slot (src/preload.rs)

One-slot cache so play starts instantly: when the selection changes, the client asks the server
to build the selected track's `AudioSource` ahead of play. The protocol is generation-based and
small: `begin(path)` bumps a generation; a finished build calls `set(generation, path, source)`
and is **dropped** if anything newer began or a `clear()` happened meanwhile; `take_if(path)`
consumes the entry only on an exact path match; `PlayerStart` on a miss clears the slot before
building fresh (so an in-flight prime can't land as a stale duplicate). Memory ceiling is by
construction the playing track plus one. The module's tests encode every race we could think of —
extend them before changing the protocol.

## The flusher and playback protection

Tag edits land in SQLite first; a background flush worker (spawned in `start_all`, logic in
`onetagger-db`) writes them into the audio files. Writing into a file that rodio is streaming
would be a disaster, so `set_currently_playing(path)` (socket.rs) records the active path and the
flusher skips it; `PlayerStart`/`PlayerLoad` set it before building a source. Flush events also
`clear()` the preload slot, since a flushed file invalidates a decoded source. `flush_wake` is an
mpsc sender any handler can poke to wake the worker early.

## Media bridge (src/media.rs)

The shell owns the OS media integration (souvlaki must live on the main thread with the event
loop); this module is the glue. The websocket layer pushes the client's player state in through a
registered sink (`register_media_sink`, called by the shell before `start_all`; a no-op in server
mode), remembering the last state so a late-joining mini window is brought up to date. OS media
commands come back through `media_key(key, position_ms)`, which broadcasts a `mediaKey` message
to every client — the main window executes transport, per mirror-don't-drive.

## Debugging: start with the app log

`~/Library/Preferences/com.OneTagger.OneTagger/onetagger.log` (via `onetagger-shared`'s fern
setup; rotated to `.old` at launch once it passes 8MB). It interleaves server logs with **client diag events** —
the client ships structured breadcrumbs (`appBoot`, `playStart`, `tagAdd`, JS errors) over the
websocket and they land here with a `client:` prefix. One file tells the whole story; both audio
bugs above were diagnosed almost entirely from it.

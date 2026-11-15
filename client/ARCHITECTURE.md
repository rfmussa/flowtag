# client — the Vue app

The face of FlowTag: Vue 3 + Vite + Tailwind v4, built to `client/dist` and embedded into the
server binary (see [crates/onetagger-ui/ARCHITECTURE.md](../crates/onetagger-ui/ARCHITECTURE.md)
for the embed gotcha). Views live in `client/src/views` (`QuickTag.vue` is FlowTag proper,
`MiniPlayer.vue` the mini window), FlowTag components in `client/src/components/flowtag`, app
logic in `client/src/scripts`. Pure, extracted logic lives in `client/src/lib` with vitest suites
beside it (`pnpm test`, 16 files / ~142 tests) — the pattern throughout: `lib/` holds pure
testable functions (end-of-track decisions, commit flow, quit flow, keybind reservations),
`scripts/` holds the stateful wiring that calls them. New decision logic goes in `lib/` with a
test.

## DECISION: the OneTagger singleton (scripts/onetagger.ts)

The whole app hangs off one `OneTagger` instance — websocket, `Player`, `Settings`, every ref.
Access is `get1t()`, which returns `OneTagger.get()`: construct at most once, return the
instance ever after. The hard lesson recorded here: the old guard was `new OneTagger()` with an
early-return in the constructor, and JS class field initializers run **before** the constructor
body — so every call built a complete throwaway app object (Player and all) before the early
return could discard it. Each throwaway was then pinned forever by its Player's uncleared 150ms
`setInterval`: ~1,800 leaked app objects per second, webview deaths around 17GB (and the thing
that originally sank the Electron shell experiment). `get1t()` must never `new` per call.
*Revisit if:* never.

## The websocket

ONE socket for the app (`wsUrl()` in `scripts/utils.ts`; dev mode targets `:36913` directly,
production uses the serving host), opened in the `OneTagger` constructor. There is **no
reconnect logic**: a `close` event routes to the same handler as an error, so a dropped socket
means a dead app behind an error dialog until reload. Known gap, worth fixing someday — the
server already survives it fine (the deck is server-side and keeps playing). The only other
sockets are throwaway per-waveform connections in `scripts/player.ts`.

## Audio is server-side

The deck is rodio, in the server process. The client `Player` (`scripts/player.ts`) is a remote
control plus a clock: it sends `playerStart`/`playerPlay`/`playerSeek`… over the socket and
advances `position` by 150ms per tick of its one interval. End-of-track *decisions* are pure
functions in `lib/player.ts`: the server's exact `playerEnded` event is the authoritative
trigger, and the clock only acts as a safety net `CLOCK_END_GRACE_MS` (30s) past the stated
duration, because duration can be a short bitrate estimate on headerless VBR files.

`playerEnded` rides the server's shared broadcast, so **every window receives it**. Only the
main window acts: the handler in `scripts/onetagger.ts` is guarded by
`window.location.hash.startsWith('#/mini')`, and the `mediaKey` handler in `scripts/tagstore.ts`
carries the same guard. That is the mirror-don't-drive rule for secondary windows — the mini
renders state and sends intents, it never executes transport, or a shared deck would get every
command twice.

## Theming

All chrome color flows through `--ft-*` tokens defined in `client/src/style/flowtag.scss`; a
theme is a value set for those tokens, applied as a `th-<id>` class on `<html>` so portaled
layers inherit it. The registry is `scripts/themes.ts`: graphite (the token defaults), carbon,
umber, porcelain, bluehour, uv, signal, phosphor. The Settings → Appearance picker
(`components/flowtag/ThemePicker.vue`) auditions themes the way you audition tracks: focus or
hover previews the whole app live, click or Enter commits, leaving reverts to the committed
theme. Porcelain, the one light theme, compensates data colors for the light ground: Camelot
key text runs through `color-mix` on the component-set `--kc` variable, and chip dots get a
hairline ring. The colorful shelf (Blue Hour, UV, Signal, Phosphor) puts hue **only in the
interaction layer** — focus, washes, selection, waveform — so tag dots and Camelot key colors
keep carrying the data; that is DESIGN.md's Data-Only Color Rule.

Exactly two authored motions exist: the 240ms theme-switch crossfade (`applyTheme(id, true)`,
skipped under reduced motion) and the keycap press echo (`scripts/keycapecho.ts` — any key fired
outside a text field dips every visible matching `.ft-kbd` keycap for a beat).

## The keyboard map (scripts/flowkeys.ts, scripts/settings.ts)

`handleFlowKeys` is the QuickTag-view keymap, with hard constraints on what may move:

- **Digits 0–9** seek to that tenth of the track, YouTube-style. Not remappable.
- **Fixed:** Space (play the selected row, or toggle), ↑/↓ select (Shift extends), ←/→ seek
  −10s/+30s, Esc clears the multi-select, ⌘Z undo, ⌘Enter the commit sheet. Tab is deliberately
  unbound so focus navigation survives.
- **Letters** are the remappable layer: ten named actions (defaults J/K prev/next, T tag input,
  F filter, D done, E edit, M lookup, B sidebar, S stage, X skip) plus per-tag hotkeys from the
  pool; `reservedFlowtagCodes()` keeps the two sets from colliding.
- **The Shift layer** is mostly free. ⇧Space plays from the drop marker (analysis on);
  ⇧1–⇧5 set the star rating on the selection (pressing the rating every selected track already
  has clears it; held keys don't repeat). A shifted digit never seeks: ⇧6–⇧0 fall through to
  per-tag hotkeys. A tag hotkey or action bind saved on ⇧1–⇧5 before ratings existed keeps its
  chord; new binds there are refused in Settings and Manage tags.

## Windows and the shell

Two windows, one route split: main, and mini on `#/mini` (`views/MiniPlayer.vue`) — just a
second client of the same server. The desktop shell is detected with
`typeof window.ipc !== 'undefined'` and spoken to via `window.ipc.postMessage(...)`: `devtools`,
`exit` (the quit handshake's reply), `toggleMini`, and from the mini `closeMini` / `dragMini`
(frameless windows drag themselves). In a plain browser none of that exists and the related
UI hides. Client diagnostics (`scripts/diag.ts`) ship structured breadcrumbs over the websocket
into the server's log file, so one file tells the whole story — see the ui crate doc.

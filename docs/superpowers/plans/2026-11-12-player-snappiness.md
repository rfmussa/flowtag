# Player snappiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Play starts and seeks land instantly for every format, with timing logs proving it.

**Architecture:** A shared `PcmPlayback` source (seekable iterator over `Arc<DecodedPcm>`) replaces the AIFF and ALAC streaming paths; AIFF/ALAC decode once into a cached buffer. The socket layer gains a one-slot preload primed on row selection, and passes the DB's duration into the sources so lofty reads leave the hot path. `debug!` timing logs land first and stay.

**Tech Stack:** Rust (rodio 0.20, pacmog, alac crate, lofty), axum WebSocket, Vue client (one debounced send).

**Spec:** `docs/superpowers/specs/2026-11-12-player-snappiness-design.md`

## Global Constraints

- No behaviour change to play/pause/volume semantics or the WS protocol beyond the new `playerPreload` action.
- The player thread never blocks on preloading (decode on `tokio::task::spawn_blocking`).
- Memory ceiling: the playing track + one preloaded track decoded in RAM.
- A corrupt file fails exactly as today (error, never a panic): keep single `catch_unwind` guards around pacmog calls (the crate panics).
- `rodio` stays at 0.20; `Source<Item = i16>` is the output everywhere, as today.
- Timing logs are `debug!`, named exactly: `player source: {path:?} built in {ms}ms`, `player seek: {pos}ms in_place={bool} took {ms}ms`, `player preload: {path:?} ready in {ms}ms`.
- Commits: `<type>: <description>` + exactly: Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>

## Review Focus

1. Pressing play immediately after selecting (preload slot still filling on the worker) must play correctly without double-decoding or blocking the handler → Task 5 test `start_while_preload_pending_loads_fresh_without_blocking`.
2. Seeking past the end of an AIFF buffer (waveform click at the far right) must clamp and end the track, not panic or wrap → Task 2 test `try_seek_past_eof_clamps_to_end`.
3. A file rewritten by the flusher between preload and play must not play stale audio → Task 5 test `flush_event_clears_preload_slot`.
4. A DB row with NULL duration (e.g. never scanned) must fall back to the lofty read, not a 0-length player → Task 4 test `missing_duration_falls_back_to_lofty`.
5. A Seek arriving before any Load (no source yet) must still reply and not deadlock — behaviour exists; pin it → Task 1 test `seek_without_source_replies_false`.

---

### Task 1: Timing instrumentation + seek-reply pin

**Files:**
- Modify: `crates/onetagger-player/src/lib.rs` (Seek and Load arms; `AudioPlayer::new` thread), `crates/onetagger-ui/src/socket.rs` (PlayerStart/PlayerLoad handlers: wrap `AudioSources::from_path` + `load_file` in an `Instant` and `debug!` the source-built line)
- Test: `crates/onetagger-player/tests/player.rs` (new)

**Interfaces:**
- Produces: the three exact log lines from Global Constraints (seek log in the Seek arm covers both in-place and fallback, with `in_place` telling which); no API changes.

- [ ] **Step 1: Write the failing test** `seek_without_source_replies_false` — `AudioPlayer::new()`, call `seek(1000)` with nothing loaded, assert it returns `false` within the call (the test completing at all pins no-deadlock). Note in the test why no audio device may exist in CI: construct the player; if `OutputStream::try_default` panics on a device-less machine, `AudioPlayer::new` panics — guard the test with a device check? NO — macOS dev machine has a device; keep the test plain and mark `#[ignore]` with a comment if it proves flaky on CI (this repo has no CI).
- [ ] **Step 2: Run** `cargo test -p onetagger-player` → FAIL (no tests dir yet / test fails to compile).
- [ ] **Step 3: Implement** the logs (Instant around `audio_source.get_source()` in the Load arm; around the whole Seek arm with `in_place` flag; socket handlers time `from_path`).
- [ ] **Step 4: Run** `cargo test -p onetagger-player && cargo build -p onetagger-ui` → pass.
- [ ] **Step 5: Commit** `feat: player timing logs`

---

### Task 2: Shared PCM playback + AIFF decode-once with in-place seek

**Files:**
- Create: `crates/onetagger-player/src/pcm.rs`; declare `pub(crate) mod pcm;` in lib.rs
- Modify: `crates/onetagger-player/src/aiff.rs`
- Test: inline `#[cfg(test)]` in pcm.rs and aiff.rs

**Interfaces:**
- Produces (pcm.rs):
  - `pub struct DecodedPcm { pub channels: u16, pub sample_rate: u32, pub samples: Vec<i16> }`
  - `pub struct PcmPlayback { /* Arc<DecodedPcm> + index */ }` with `PcmPlayback::new(data: Arc<DecodedPcm>) -> PcmPlayback`; implements `Iterator<Item = i16>` and `rodio::Source<Item = i16>` (`channels`, `sample_rate`, `total_duration` = Some(computed), `current_frame_len` = None) and **`try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError>`** — `index = min(frame * channels, samples.len())` where `frame = pos.as_millis * sample_rate / 1000`; always `Ok(())`.
- Produces (aiff.rs): `AIFFSource` gains `cache: std::sync::Mutex<Option<Arc<DecodedPcm>>>`; `get_source` fills the cache once via `decode_aiff(data: &[u8]) -> Result<DecodedPcm, Error>` (ONE `catch_unwind` around the whole pacmog read loop, errors preserved) and returns `Box::new(PcmPlayback::new(arc))`.
- Consumes: nothing new.

- [ ] **Step 1: Write failing tests**
  - pcm.rs: `try_seek_lands_on_exact_frame` (DecodedPcm 2ch/1000Hz/8 frames with distinct values; seek to 3ms → next two samples are frame 3's); `try_seek_past_eof_clamps_to_end` (seek 10s → iterator returns None); `source_properties_match` (channels/sample_rate/total_duration).
  - aiff.rs: test helper `fn tiny_aiff(frames: u16) -> Vec<u8>` building a minimal valid AIFF (FORM/COMM/SSND, 2ch 16-bit 8kHz, distinct sample values; big-endian — pacmog must parse it); `bulk_decode_matches_samples` (decode_aiff gives the written values); `get_source_caches_decode` (two get_source calls; assert `Arc::ptr_eq` of the cached arcs via a test accessor or by exposing `#[cfg(test)] fn cached(&self) -> Option<Arc<DecodedPcm>>`).
- [ ] **Step 2: Run** `cargo test -p onetagger-player` → FAIL.
- [ ] **Step 3: Implement** pcm.rs, decode_aiff, the cache, try_seek.
- [ ] **Step 4: Run** `cargo test -p onetagger-player` → pass; the old per-sample loop is gone.
- [ ] **Step 5: Commit** `feat: AIFF decodes once and seeks in place`

---

### Task 3: ALAC through the same buffer

**Files:**
- Modify: `crates/onetagger-player/src/alac.rs`
- Test: inline in alac.rs (no ALAC fixture is practical — the alac crate only decodes; see ruling)

**Interfaces:**
- Consumes: `DecodedPcm`, `PcmPlayback` from Task 2.
- Produces: `ALACSource` restructured like AIFFSource: `new(path)` keeps path + `stream_info` (no change to what it reads); `get_source()` decodes the WHOLE stream once through the existing `Samples` iterator into a cached `Arc<DecodedPcm>` (i32 samples → i16 via `>> 16` on 32-bit, `as i16` on 16-bit — read `stream_info.bit_depth()`), returns `PcmPlayback`. RULING: no packet-skip variant — the buffer path reuses tested machinery and ALAC files are the rarest in the library; no ALAC fixture test exists, so the conversion is pinned by a unit test on the sample-conversion helper only: `pub(crate) fn alac_sample_to_i16(s: i32, bit_depth: u8) -> i16` with `converts_16_and_32_bit_depths`.
- `AudioSource::duration` for ALAC stays as today.

- [ ] **Step 1: Write failing test** `converts_16_and_32_bit_depths` (16-bit: 0x1234 → 0x1234; 32-bit: 0x1234_0000 → 0x1234; negative values).
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement** helper + buffer + cache + PcmPlayback return. **Step 4:** `cargo test -p onetagger-player` → pass. **Step 5: Commit** `feat: ALAC decodes once and seeks in place`

---

### Task 4: Duration from the DB

**Files:**
- Modify: `crates/onetagger-player/src/lib.rs` (`AudioSources::from_path_with_duration(path, Option<u128>)`; `from_path` delegates with None), all five source files (`::new(path)` → keep; add `new_with_duration(path, Option<u128>)` that skips the lofty read when `Some`; `new` delegates), `crates/onetagger-db/src/tracks.rs` (`pub fn duration_ms_for_path(db: &Db, path: &str) -> Result<Option<i64>, Error>` — present tracks only), `crates/onetagger-ui/src/socket.rs` (PlayerStart :584 and PlayerLoad :615 look up the duration and call `from_path_with_duration`)
- Test: tracks.rs inline; player tests

**Interfaces:**
- Produces: the two signatures above; later Task 5 uses `from_path_with_duration`.

- [ ] **Step 1: Write failing tests** — tracks.rs: `duration_for_known_path_and_none_for_missing`; player (aiff.rs): `supplied_duration_skips_file_metadata` — `AIFFSource::new_with_duration(path_to_tiny_aiff_file, Some(1234))` (write the tiny fixture to a tempfile) → `duration() == 1234` without lofty reading tags (assert by giving the file NO valid tag section — the tiny AIFF has none — `new` with None would still work since lofty reads properties; simply assert duration equals the supplied value).
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4:** `cargo test -p onetagger-player -p onetagger-db && cargo build` → pass. **Step 5: Commit** `feat: player takes track duration from the library`

---

### Task 5: Preload on selection

**Files:**
- Modify: `crates/onetagger-ui/src/socket.rs` (Action `PlayerPreload { path: PathBuf }`; a shared slot; PlayerStart/PlayerLoad consume it; deserialize test), `crates/onetagger-ui/src/lib.rs` (slot created beside the player; flush event hook clears it), `client/src/scripts/tagstore.ts` (watch the selected track, debounce 150 ms, `$1t.send('playerPreload', { path })`; skip when the path is already loaded in `$1t.player`), client test in `client/src/lib/staging.test.ts` or a new small test file
- Test: socket.rs tests + a slot unit test

**Interfaces:**
- Consumes: `AudioSources::from_path_with_duration` (Task 4).
- Produces:
  - `pub(crate) struct PreloadSlot(Arc<Mutex<Option<(PathBuf, Box<dyn AudioSource + Send>)>>>)` with `set`, `take_if(path) -> Option<Box<dyn AudioSource + Send>>`, `clear()` — in socket.rs or a tiny `preload.rs` in onetagger-ui (implementer's choice, say which).
  - `PlayerPreload` handler: spawn_blocking `from_path_with_duration` + prime the decode cache by calling `get_source()` once and dropping the result (fills the AIFF/ALAC Arc; cheap no-op for symphonia formats), then `slot.set(path, source)`; logs the `player preload:` line. It must NOT call `set_currently_playing`.
  - PlayerStart/PlayerLoad: `slot.take_if(&path)` first; hit → use it (log `player source: … built in 0ms (preloaded)`), miss → load as today.
  - RULING: ANY flush event clears the whole slot (conservative; FlushEvent carries only track_id) — wire in lib.rs's flusher `on_event` closure.
- Client: selection watcher sends at most one preload per 150 ms settle; never for the currently playing path.

- [ ] **Step 1: Write failing tests** — socket.rs: `player_actions_deserialize` gains `playerPreload`; slot unit tests `take_if_matches_path_once`, `flush_event_clears_preload_slot` (call the clear hook, slot empty), `start_while_preload_pending_loads_fresh_without_blocking` (slot empty at take_if → None; the handler path falls through — test the slot semantics, not the full handler). Client: `preload_send_debounces_and_skips_playing` on the watcher helper (extract a pure decision fn `shouldPreload(selPath, playingPath) -> boolean`).
- [ ] **Step 2: Run** `cargo test -p onetagger-ui` + `pnpm test` → FAIL. **Step 3: Implement.** **Step 4:** `cargo test -p onetagger-ui && cargo build && cd client && pnpm test && pnpm run build` → pass. **Step 5: Commit** `feat: preload the selected track for instant play`

---

### Task 6: Install + live check (with the user)

- [ ] **Step 1:** Full suites green (`cargo test -p onetagger-player -p onetagger-db -p onetagger-ui -p onetagger-tangle`, client tests + build); reinstall FlowTag.app (existing script); DB untouched by this feature (no migrations).
- [ ] **Step 2:** With debug logging, capture BEFORE numbers from the previous build are not available — instead capture the new build's numbers and compare formats: play-start and three seeks on one long AIFF, one MP3, one FLAC; expect AIFF first play to decode once (log line), then seeks `in_place=true took ≤1ms`, and preloaded play `built in 0ms (preloaded)`.
- [ ] **Step 3:** Feel-check with the user: J/K through tracks then Space (instant), waveform clicks on an AIFF (instant after first play), MP3/FLAC unchanged-or-better.
- [ ] **Step 4:** Push the branch.

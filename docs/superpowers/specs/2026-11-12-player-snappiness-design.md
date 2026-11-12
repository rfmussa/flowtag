# Player snappiness — design

Date: 2026-11-12 · Status: approved in conversation, awaiting spec review
Research: `research_notes/Player snappiness techniques/techniques.md` (Mixxx CachingReader, foobar2000 decode-ahead, symphonia/rodio capabilities)

## Goal

Press play → sound at once; click the waveform → the position changes at once,
for every format in the library (MP3, FLAC, M4A/AAC, ALAC, AIFF, WAV). One
user, macOS, SSD; the player lives in `crates/onetagger-player` (rodio 0.20 +
symphonia 0.5 for MP3/FLAC/WAV/M4A; custom sources for AIFF and ALAC).

## Diagnosis (verified in code)

| Problem | Where | Effect |
|---|---|---|
| AIFF play decodes the WHOLE file before sound — per sample, each wrapped in `catch_unwind` | aiff.rs `AIFFDecoder::load` (`get_source()` runs it on every call) | seconds of silence on play for long AIFFs |
| AIFF/ALAC seek: `sink.try_seek` fails (custom sources lack `try_seek`), fallback re-creates the source (→ full re-decode for AIFF) then skips samples one by one | lib.rs Seek arm; aiff.rs/alac.rs | multi-second seeks; the PCM buffer that would make seek free already exists |
| Every play re-reads file metadata for the duration | mp3/flac/wav/mp4/aiff `::new` via lofty | small fixed cost; the DB already stores duration_ms |
| Load/decode happen only after the user presses play | socket.rs PlayerStart/PlayerLoad → from_path → get_source on the player thread | perceived latency = everything serial after the keypress |

MP3/FLAC/WAV/M4A already seek in place via symphonia (`Decoder` + `try_seek`). The output stream is created once and reused (good).

## Changes

### 1. Instrument first (and keep the instrumentation)
`debug!` timing logs in the player and socket paths: `player load: {path} source_ms={} (format)`, `player first-sound gap: {}ms`, `player seek: pos={} in_place|fallback took={}ms`. The live check reads before/after numbers from onetagger.log with the existing debug logging.

### 2. AIFF: decode once, seek in RAM
- `AIFFSource` caches the decoded PCM: `Arc<DecodedPcm>` (`Vec<i16>` + specs), filled on first `get_source()` and reused by later calls (plays and fallback seeks stop re-decoding).
- Replace the per-sample `catch_unwind` decode loop with one `catch_unwind` around a bulk decode loop (same pacmog API, one guard; honour its errors).
- `AIFFDecoder` implements `Source::try_seek`: `index = frame(pos) * channels` clamped to the buffer, `Ok(())`. Seeking becomes an index assignment.

### 3. ALAC: implement `try_seek` by packet skip
`ALACSource` keeps `path` + `stream_info`; `try_seek` re-opens a `Reader` and skips PACKETS (not samples) to the target frame using the stream's frames-per-packet, then decodes forward within the packet. If the `alac` crate exposes no packet iteration that beats sample-skipping, fall back to: decode-to-buffer on first `get_source()` exactly like AIFF (ALAC files are small enough; same Arc cache). Decide by measurement in the task; either way `try_seek` must succeed so the sink seek path is used and the source is never re-created.

### 4. Preload on selection
- New `Action::PlayerPreload { path }` sent by the client when the track-list selection changes (debounced 150 ms, only when the player isn't currently playing that path).
- Backend keeps `preloaded: Option<(PathBuf, Box<dyn AudioSource + Send>)>` in the socket context (one slot). `PlayerPreload` runs `AudioSources::from_path` + (for AIFF/ALAC) triggers the decode cache fill on a `spawn_blocking` thread.
- `PlayerStart`/`PlayerLoad` take the preloaded source when the path matches; otherwise load as today. The slot is dropped on mismatch (one track's PCM in memory at most, beyond the playing one).
- `set_currently_playing` protection unchanged; preloading must NOT mark the file as playing (the flusher may still write a preloaded-but-not-playing file; on flush the preload slot for that path is invalidated — hook the existing flush event).

### 5. Duration from the DB
`PlayerStart`/`PlayerLoad`/`PlayerPreload` pass the DB's `duration_ms` (tracks table, by path) into the sources so `::new` skips the lofty read when a duration is supplied (keep lofty as fallback for unknown paths, e.g. lookup previews). Signature: `AudioSources::from_path_with_duration(path, Option<u128>)`; `from_path` delegates.

## Explicitly out of scope
Upgrading rodio (0.21+ API churn; revisit later — the TODO comment stays updated), full-track decode-ahead for symphonia formats (measure first; their in-place seek is already fast), gapless, crossfade, LRU of multiple decoded tracks, waveform changes, client-side player mode.

## Constraints
- No behaviour change to play/pause/volume semantics or the WS protocol beyond the new `playerPreload` action.
- The player thread must never block on preloading (decode on a worker; the Arc cache makes the handoff cheap).
- Memory: at most the playing track + one preloaded track decoded in RAM (AIFF ~60–130 MB worst case each — acceptable).
- A corrupt file must fail exactly as today (error, no panic): the single `catch_unwind` guards stay.

## Testing (TDD)
- aiff.rs: bulk decode equals the old per-sample decode on a fixture (generate a tiny AIFF in the test); `try_seek` lands on the exact frame (seek then next() samples match buffer offset); seek past EOF clamps; cache: second `get_source()` does not re-read the file (fs metadata/counter via a test hook or timing-free assertion on Arc pointer equality).
- alac.rs: `try_seek` correctness on a tiny fixture (whichever strategy wins); never re-creates the source on seek (lib.rs fallback not taken — assert via the seek arm returning in-place true, testable through AudioPlayer with a fixture file).
- lib.rs: seek on a source with try_seek never rebuilds the sink (observable: position preserved ordering, or a unit test on the Seek arm with a mock source counting get_source calls).
- socket: `playerPreload` deserializes; preload slot used on matching PlayerStart (get_source call count on a counting mock), dropped on mismatch, invalidated on flush of the same path.
- from_path_with_duration skips lofty (counting/timing-free: a path with intentionally broken tags but valid audio loads fine when a duration is supplied).
- Live check: before/after timings from the log for one long AIFF (play-start and three seeks), one MP3, one FLAC; feel-check with the user.

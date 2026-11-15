# onetagger-player — the deck

Audio playback for the whole app. One `AudioPlayer` instance exists per process (the singleton
"deck" lives one layer up, in `onetagger-ui`'s socket module); this crate is the machinery inside
it. See the repo-root [ARCHITECTURE.md](../../ARCHITECTURE.md) for the One-Deck Invariant.

## Shape

`AudioPlayer::new()` (src/lib.rs) spawns one playback thread that owns the `rodio::OutputStream`
and the one `Sink` for its whole life. Everything else talks to it over an mpsc channel of
`PlayerAction`s: `Load`, `Play`, `Pause`, `Stop`, `Seek`, `Volume`, `RebindOutput`. `Seek` is the
only synchronous call — it blocks on a reply channel so the caller learns whether playback
resumed. Push events flow the other way: `take_events()` yields a receiver (once) carrying
`PlayerEvent::Ended`.

The thread's loop is a `recv_timeout(200ms)`: actions when they arrive, and on each quiet tick a
dry-sink check for end-of-track detection.

## DECISION: one sink for the deck's lifetime

Every `Load` used to build a brand-new `Sink` and drop the old one after `stop()`. That was the
undead-sink bug: a replaced sink can outlive its stop by a beat and keep feeding the mixer under
the new track. The user heard it as *"plays the previous track until I seek forward"*, reliably
provoked by rapid A/B track comparison. The forensic clue that pinned it: the seek that cured the
audio was an **in-place** `try_seek` — meaning the current sink already held the correct track,
so the wrong sound had to be a predecessor sink that had not died. The same mechanism was half of
the earlier two-tracks-at-once incident (per-connection players were the other half).

Now `Load` calls `sink.clear()` — stop, empty, pause, same sink — and appends the new source
into it. A predecessor sink never exists. The only place a sink is rebuilt is a device rebind,
where the old `OutputStream` is dropped and therefore silenced at the device level, and the seek
fallback follows the same clear-and-reuse rule, resuming explicitly afterwards because `clear()`
always leaves the sink paused.

*Revisit if:* rodio changes `Sink::clear()` semantics (we rely on stop+empty+pause-in-place), or
a cue/preview deck is built — that second audible source must be a deliberate, named second deck
with its own sink and output, never an accident of connection count or load churn.

## Device rebind and sleep

rodio never follows macOS default-output changes on its own, so the thread registers a CoreAudio
listener (`output_watch.rs`) that posts `RebindOutput`. Rebind captures the position, opens a
fresh `OutputStream` + sink, re-creates the source from the stored `AudioSource`, seeks back, and
resumes if it was playing.

System sleep kills the CoreAudio stream *without* renaming the device, so there is a sleep
detector: the monotonic clock stops during sleep and the wall clock does not, and skew between
the two across consecutive actions means a sleep happened. A `Play` arriving after detected sleep
re-queues `[RebindOutput, Play]` so resume never plays into a dead stream. `Load` performs the
same check inline (`should_rebind` + slept).

*Revisit if:* false-positive sleep detection ever shows up (NTP step adjustments could mimic
skew) — the cost today is only a harmless ~220ms rebind.

## Sources and the PCM cache

`AudioSources::from_path` picks a per-format `AudioSource` implementation: symphonia streaming
decoders for FLAC/MP3/MP4/WAV, rodio's built-in vorbis decoder for OGG (`ogg.rs`,
`Decoder::new_vorbis`), and for AIFF (`aiff.rs`, via pacmog) and ALAC (`alac.rs`) a
**per-source** decoded-PCM path (`pcm.rs`). `DecodedPcm` is the fully decoded interleaved i16
buffer behind an `Arc`; `PcmPlayback` is a trivially seekable rodio source over it. ALAC starts a
streaming decoder instantly while `CacheFill` decodes the whole file in the background; once the
cache lands, seeks and replays use the PCM. The cache is owned by its `AudioSource` instance and
keyed to nothing else — two tracks can never share or contaminate a buffer (this was verified,
not assumed, while hunting the undead-sink bug).

`duration_ms` can be passed in from the library DB to skip re-reading file metadata on load — the
preload path uses this.

## Seek

Two tiers in the `Seek` arm. In-place first: `sink.try_seek(pos)` — symphonia sources and
`PcmPlayback` all support it, making seeks near-instant. Fallback for anything that refuses:
clear the sink, re-create the source, skip `rate × channels × pos` samples by iteration, append,
and resume if it was playing. The fallback is also what guarantees a seek lands on the *stored*
source — historically useful as a self-healing path, and the reason the undead-sink bug looked
like "fixed by seeking".

## End of track

Two signals, by design. The exact one: the 200ms tick notices the sink ran dry
(`should_emit_ended`: a source is loaded, not paused, sink empty, not already sent) and pushes
`PlayerEvent::Ended`, which `onetagger-ui` broadcasts to every window as `playerEnded`. The
client treats this as the authoritative trigger and keeps only a grace-period wall-clock check as
a safety net (see [client/ARCHITECTURE.md](../../client/ARCHITECTURE.md)). `ended_sent` arms once
per load so a track ends exactly once; an explicit `Stop` disarms it so stopping is never
mistaken for finishing.

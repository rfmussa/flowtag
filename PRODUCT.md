# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

A single primary user: the repo owner, a DJ managing their own music library. This fork is a personal tool — product decisions optimize for one experienced power user who knows the app deeply. No onboarding for newcomers, upstream users, or other audiences is required.

## Product Purpose

A cross-platform desktop music tagger (Rust backend, Vue 3 + Quasar webview UI), forked from Marekkon5/onetagger. The fork's center of gravity is **FlowTag**: a full library-management workflow — scanning, organizing, tagging, and maintaining the whole collection over time. Tagging tracks is one part of that broader job, not the whole product. Success means the owner's library stays organized, consistently tagged, and ready for DJ use with minimal friction per session.

## Positioning

Upstream OneTagger is a metadata fetcher and quick-tag editor for DJs broadly. This fork replaces the Quick Tag system with FlowTag: a library-database-backed management view (SQLite via the `onetagger-db` crate) with keyboard-driven tagging, tag/group management, library scanning, a background flusher that writes DB tags into audio files, and Rekordbox comment export. The mechanism a neighboring tool could not copy: it is tuned to exactly one user's workflow, with no generalization tax.

## Operating Context

- Desktop app (macOS primary dev machine; upstream supports Windows/Linux) running a Vue/Quasar UI inside a Rust webview.
- Workflow touches real audio files (MP3, AIFF, FLAC, M4A) on disk; tags are written into the files by a background flusher.
- Downstream DJ software (e.g. Rekordbox via comment export) consumes the tagged output.
- Upstream views still present: Autotagger (metadata fetching from Beatport, Traxsource, Discogs, MusicBrainz, Spotify, etc.), Audio Features, Tag Editor, Renamer.

## Capabilities and Constraints

- **Keyboard-first operation is a binding constraint:** FlowTag must stay fully drivable from the keyboard; the mouse is secondary. Future work must preserve this.
- FlowTag capabilities confirmed in code: library scan pipeline, SQLite library DB, tag/group management UI, multi-select, sorting, tag filtering, undo, waveform player with real seeking and cached waveforms, Rekordbox comment export with confirm dialog, background DB→file tag flusher.
- Not confirmed as constraints (explicitly left unselected by the owner): staying mergeable with upstream, a fixed Rekordbox comment format, and files-as-sole-source-of-truth. Treat these as open, not binding.

## Brand Commitments

Inherits upstream OneTagger name and logo assets (`assets/`). No fork-specific brand commitments were made binding.

## Evidence on Hand

- Working implementation: `client/src/views/QuickTag.vue` (hosts FlowTag), `client/src/components/flowtag/` (TrackList, TagPalette, TagManager, GroupManager, ScanStatus, FlushIndicator, etc.), `crates/onetagger-db`.
- Upstream README, website (onetagger.github.io), and logo assets.
- No testimonials, metrics, or external users — and none should be fabricated; this is a personal tool.

## Product Principles

1. **One user, zero generalization tax** — build for the owner's actual workflow, not hypothetical users; skip onboarding, marketing, and discoverability work.
2. **Keyboard-first, always** — every FlowTag action reachable and fast from the keyboard; pointer interactions are a convenience layer.
3. **The library is the product** — features earn their place by keeping the whole collection organized and maintained over time, not just by speeding up single-track edits.
4. **Respect the audio files** — tagging operations ultimately land in real files on disk; destructive or lossy writes are the costliest failure.
5. **Session flow over feature breadth** — minimize friction inside a tagging/maintenance session (instant playback, undo, visible flush state) before adding new capability.

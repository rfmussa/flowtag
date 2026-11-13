import { describe, expect, it } from 'vitest';
import { endOfTrackAction, shouldPreload, trackEndAction } from './player';

describe('shouldPreload', () => {
    it('preload_skips_null_and_playing_path', () => {
        expect(shouldPreload(null, null)).toBe(false);
        expect(shouldPreload(null, '/m/a.mp3')).toBe(false);
        expect(shouldPreload('/m/a.mp3', '/m/a.mp3')).toBe(false);
        expect(shouldPreload('/m/a.mp3', '/m/b.mp3')).toBe(true);
        expect(shouldPreload('/m/a.mp3', null)).toBe(true);
    });
});

describe('endOfTrackAction', () => {
    const atEnd = { playing: true, position: 200_100, duration: 200_000, autoPlayNext: true, onQuickTag: true, multiSelect: false };

    it('does_nothing_while_the_track_still_runs', () => {
        expect(endOfTrackAction({ ...atEnd, position: 100_000 })).toBe('none');
    });

    it('does_nothing_when_paused_even_past_the_end', () => {
        // The old check refired every tick after pausing, spamming playerPause
        expect(endOfTrackAction({ ...atEnd, playing: false })).toBe('none');
    });

    it('advances_once_past_the_end_on_quicktag', () => {
        expect(endOfTrackAction(atEnd)).toBe('advance');
    });

    it('pauses_instead_of_advancing_without_the_setting_route_or_with_multiselect', () => {
        expect(endOfTrackAction({ ...atEnd, autoPlayNext: false })).toBe('pause');
        expect(endOfTrackAction({ ...atEnd, onQuickTag: false })).toBe('pause');
        expect(endOfTrackAction({ ...atEnd, multiSelect: true })).toBe('pause');
    });
});

describe('server-ended vs clock fallback', () => {
    it('track_end_decision_ignores_the_clock', () => {
        const base = { autoPlayNext: true, onQuickTag: true, multiSelect: false };
        expect(trackEndAction(base)).toBe('advance');
        expect(trackEndAction({ ...base, autoPlayNext: false })).toBe('pause');
        expect(trackEndAction({ ...base, onQuickTag: false })).toBe('pause');
        expect(trackEndAction({ ...base, multiSelect: true })).toBe('pause');
    });

    it('clock_waits_out_the_grace_window', () => {
        // Stated durations can be short (bitrate estimates): the clock only
        // acts well past the stated end, leaving the exact server event to act
        const s = { playing: true, position: 210_000, duration: 200_000, autoPlayNext: true, onQuickTag: true, multiSelect: false };
        expect(endOfTrackAction({ ...s, graceMs: 30_000 })).toBe('none');
        expect(endOfTrackAction({ ...s, position: 230_100, graceMs: 30_000 })).toBe('advance');
        expect(endOfTrackAction(s)).toBe('advance');
    });
});

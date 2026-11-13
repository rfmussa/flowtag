import { describe, expect, it } from 'vitest';
import { endOfTrackAction, shouldPreload } from './player';

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

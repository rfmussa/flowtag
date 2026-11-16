import { describe, expect, it } from 'vitest';
import { nextSkip, tickIds, unreadyIds } from './skip';

describe('nextSkip', () => {
    it('single_track_toggles', () => {
        expect(nextSkip([{ staged: true, skip: false }])).toBe(true);
        expect(nextSkip([{ staged: true, skip: true }])).toBe(false);
    });
    it('unstaged_counts_as_not_skipped', () => {
        expect(nextSkip([{}])).toBe(true);
        expect(nextSkip([{ staged: false, skip: true }])).toBe(true);
    });
    it('clears_only_when_every_track_is_skipped', () => {
        const on = { staged: true, skip: true };
        expect(nextSkip([on, on, on])).toBe(false);
        expect(nextSkip([on, { staged: true, skip: false }, on])).toBe(true);
        expect(nextSkip([on, {}])).toBe(true);
    });
    it('empty_sets', () => {
        expect(nextSkip([])).toBe(true);
    });
});

describe('unreadyIds', () => {
    it('keeps_unready_rows_in_order', () => {
        const rows = [
            { trackId: 1, skip: false, ready: false },
            { trackId: 2, skip: false, ready: true },
            { trackId: 3, skip: false, ready: false },
        ];
        expect(unreadyIds(rows)).toEqual([1, 3]);
        expect(unreadyIds([])).toEqual([]);
    });
});

describe('tickIds', () => {
    it('ticks_ids_without_mutating', () => {
        const before = Object.freeze({ 1: true, 2: false });
        const after = tickIds(before, [2, 3]);
        expect(after).toEqual({ 1: true, 2: true, 3: true });
        expect(before).toEqual({ 1: true, 2: false });
    });
});

import { describe, expect, it } from 'vitest';
import { commitLabel, commitPayload, commitScope, commitScopeId, initialTicks, stageCountLabel, toggleTick, withLoadedReady } from '@/lib/commit';

const rows = [
    { trackId: 1, skip: false, ready: true },
    { trackId: 2, skip: false, ready: false },
    { trackId: 3, skip: true, ready: true },
];

describe('commit ticks', () => {
    it('ticked_defaults_ready_only', () => {
        expect(initialTicks(rows)).toEqual({ 1: true, 2: false, 3: true });
    });

    it('toggle_returns_a_new_map', () => {
        const before = initialTicks(rows);
        const after = toggleTick(before, 2);
        expect(after[2]).toBe(true);
        expect(before[2]).toBe(false);
    });

    it('commit_payload_is_ticked_ids_and_trimmed_message', () => {
        const ticks = toggleTick(initialTicks(rows), 2);
        expect(commitPayload(rows, ticks, '  evening crate  ')).toEqual({ trackIds: [1, 2, 3], message: 'evening crate' });
        expect(commitPayload(rows, { 1: false, 2: false, 3: true }, '')).toEqual({ trackIds: [3], message: '' });
    });
});

describe('ready from the loaded tracks', () => {
    it('commit_sheet_preticks_a_just_tagged_track_despite_a_stale_stage_row', () => {
        // the server row predates the tag that was just added
        const stale = [{ trackId: 1, skip: false, ready: false }, { trackId: 2, skip: false, ready: true }];
        const tracks = [
            { id: 1, tagIds: [5], skip: false },
            { id: 2, tagIds: [], skip: false },
        ];
        const rows = withLoadedReady(stale, tracks);
        expect(rows.map((r) => r.trackId)).toEqual([1, 2]);
        expect(initialTicks(rows)).toEqual({ 1: true, 2: false });
        expect(stale[0].ready).toBe(false);
    });

    it('a_loaded_skip_is_ready_and_an_unloaded_track_keeps_the_server_row', () => {
        const rows = withLoadedReady(
            [{ trackId: 1, skip: false, ready: false }, { trackId: 9, skip: false, ready: true }],
            [{ id: 1, tagIds: [], skip: true }],
        );
        expect(rows).toEqual([{ trackId: 1, skip: true, ready: true }, { trackId: 9, skip: false, ready: true }]);
    });
});

describe('stage count label', () => {
    it('stage_count_flips_to_left_past_half', () => {
        expect(stageCountLabel(3, 10)).toBe('3 / 10');
        expect(stageCountLabel(5, 10)).toBe('5 / 10');
        expect(stageCountLabel(6, 10)).toBe('4 left');
        expect(stageCountLabel(10, 10)).toBe('0 left');
        expect(stageCountLabel(0, 0)).toBe('0 / 0');
    });
});

describe('commit scope', () => {
    it('round_trips_a_commit_scope_id', () => {
        expect(commitScopeId(commitScope(42))).toBe(42);
    });

    it('rejects_non_commit_scopes', () => {
        expect(commitScopeId('all')).toBeUndefined();
        expect(commitScopeId('stage')).toBeUndefined();
        expect(commitScopeId(7)).toBeUndefined();
        expect(commitScopeId('commit:x')).toBeUndefined();
    });

    it('labels_a_commit_by_date_plus_message', () => {
        const secs = Date.UTC(2026, 10, 12, 12) / 1000; // Nov 12 2026, noon UTC
        expect(commitLabel({ createdAt: secs, message: 'deep house session' }, 'en-US')).toBe('Nov 12 · deep house session');
        expect(commitLabel({ createdAt: secs, message: '  ' }, 'en-US')).toBe('Nov 12');
    });
});

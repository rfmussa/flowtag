import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TagStore, FlowTrack } from '@/scripts/tagstore';

const send = vi.hoisted(() => vi.fn());
vi.mock('@/scripts/onetagger', () => ({ get1t: () => ({ send }) }));

function track(id: number, over: Partial<FlowTrack> = {}): FlowTrack {
    return {
        id, path: `/m/${id}.mp3`, folderId: 1, format: 'mp3', title: `T${id}`, artist: 'A',
        addedAt: 0, missing: false, needsFlush: false, tagIds: [], ...over,
    };
}

function storeWith(tracks: FlowTrack[]): TagStore {
    const s = new TagStore();
    s.tracks.value = tracks;
    return s;
}

const sentIds = (action: string): number[][] =>
    send.mock.calls.filter((c) => c[0] === action).map((c) => c[1].trackIds);

beforeEach(() => send.mockClear());

describe('stageNext50', () => {
    it('stage_next_50_picks_not_done_not_staged_in_view_order', () => {
        const tracks = [
            track(1, { doneAt: 5 }),
            track(2, { staged: true }),
            ...Array.from({ length: 60 }, (_, i) => track(10 + i)),
        ];
        const s = storeWith(tracks);
        // view order: title descending flips the library order
        s.sortBy.value = 'title';
        s.sortDesc.value = true;
        const expected = s.visibleTracks().filter((t) => t.doneAt == null && !t.staged).slice(0, 50).map((t) => t.id);
        s.stageNext50();
        expect(sentIds('stageTracks')).toEqual([expected]);
        expect(expected).toHaveLength(50);
        expect(expected).not.toContain(1);
        expect(expected).not.toContain(2);
    });

    it('sends_nothing_when_nothing_qualifies', () => {
        const s = storeWith([track(1, { doneAt: 1 }), track(2, { staged: true })]);
        s.stageNext50();
        expect(sentIds('stageTracks')).toEqual([]);
    });
});

describe('toggleStageSelection', () => {
    it('stages_the_not_staged_ones_then_unstages_when_all_staged', () => {
        const s = storeWith([track(1, { staged: true }), track(2), track(3)]);
        s.selectedId.value = 1;
        s.selectedIds.value = [2];
        s.toggleStageSelection();
        expect(sentIds('stageTracks')).toEqual([[2]]);
        expect(sentIds('unstageTracks')).toEqual([]);

        send.mockClear();
        s.tracks.value = [track(1, { staged: true }), track(2, { staged: true }), track(3)];
        s.toggleStageSelection();
        expect(sentIds('unstageTracks').map((ids) => [...ids].sort())).toEqual([[1, 2]]);
    });

    it('refetches_rows_after_the_mutation', () => {
        const s = storeWith([track(1)]);
        s.selectedId.value = 1;
        s.toggleStageSelection();
        expect(send.mock.calls.map((c) => c[0])).toEqual(['stageTracks', 'tagTracksLoad']);
    });
});

describe('stage scope helpers', () => {
    it('stageSkipCurrent_toggles_the_current_track', () => {
        const s = storeWith([track(7)]);
        s.selectedId.value = 7;
        s.stageSkipCurrent();
        expect(send.mock.calls[0]).toEqual(['stageSkip', { trackId: 7 }]);
    });

    it('selectNextStagedNotReady_walks_stage_rows_with_wrap', () => {
        const s = storeWith([track(1, { staged: true }), track(2, { staged: true }), track(3, { staged: true })]);
        s.scope.value = 'stage';
        s.stageRows.value = [
            { trackId: 1, skip: false, ready: false },
            { trackId: 2, skip: false, ready: true },
            { trackId: 3, skip: false, ready: false },
        ];
        s.selectedId.value = 1;
        s.selectNextStagedNotReady();
        expect(s.selectedId.value).toBe(3);
        s.selectNextStagedNotReady();
        expect(s.selectedId.value).toBe(1);
    });

    it('notDoneUnstagedIds_caps_and_filters', () => {
        const s = storeWith([track(1, { doneAt: 1 }), track(2, { staged: true }), track(3), track(4)]);
        expect(s.notDoneUnstagedIds([1, 2, 3, 4], 1)).toEqual([3]);
        expect(s.notDoneUnstagedIds([1, 2, 3, 4])).toEqual([3, 4]);
    });
});

describe('cursor after removal', () => {
    it('commit_keeps_cursor_on_neighbour', () => {
        const s = storeWith([track(1), track(2), track(3)]);
        s.quickFilter.value = 'notdone';
        s.selectedId.value = 1;
        s.selectOffset(1); // anchor on 2, remembers its place
        expect(s.selectedId.value).toBe(2);
        // 2 gets committed: done, so the not-done view drops it
        s.tracks.value = [track(1), track(2, { doneAt: 9 }), track(3)];
        s.selectNeighbourAfterRemoval(2);
        expect(s.selectedId.value).toBe(3);
    });

    it('wraps_to_the_top_when_the_last_row_vanishes', () => {
        const s = storeWith([track(1), track(2), track(3)]);
        s.quickFilter.value = 'notdone';
        s.selectedId.value = 2;
        s.selectOffset(1);
        expect(s.selectedId.value).toBe(3);
        s.tracks.value = [track(1), track(2), track(3, { doneAt: 9 })];
        s.selectNeighbourAfterRemoval(3);
        expect(s.selectedId.value).toBe(1);
    });

    it('is_a_no_op_while_the_track_is_still_visible_or_nothing_remains', () => {
        const s = storeWith([track(1), track(2)]);
        s.selectedId.value = 1;
        s.selectNeighbourAfterRemoval(1);
        expect(s.selectedId.value).toBe(1);
        s.quickFilter.value = 'notdone';
        s.tracks.value = [track(1, { doneAt: 1 }), track(2, { doneAt: 1 })];
        s.selectNeighbourAfterRemoval(1);
        expect(s.selectedId.value).toBe(1);
    });
});

describe('commit fallback', () => {
    it('falls_back_to_all_when_a_commit_empties_the_stage_in_stage_scope', () => {
        const s = storeWith([track(1)]);
        s.scope.value = 'stage';
        s.handleMessage({ action: 'stageCommitted', commit: { id: 1, createdAt: 0, message: '', trackCount: 1 } });
        s.handleMessage({ action: 'stageState', ready: 0, total: 0, stage: [] });
        expect(s.scope.value).toBe('all');
    });

    it('stays_in_stage_scope_when_the_stage_empties_without_a_commit', () => {
        const s = storeWith([track(1)]);
        s.scope.value = 'stage';
        s.handleMessage({ action: 'stageState', ready: 0, total: 0, stage: [] });
        expect(s.scope.value).toBe('stage');
    });
});

describe('commitStageAndWait', () => {
    it('resolves_when_the_commit_is_confirmed', async () => {
        const s = storeWith([track(1)]);
        const p = s.commitStageAndWait([1], 5000);
        s.handleMessage({ action: 'stageCommitted', commit: { id: 9, createdAt: 0, message: '', trackCount: 1 } });
        await expect(p).resolves.toBeUndefined();
    });

    it('resolves_early_on_a_server_error', async () => {
        const s = storeWith([track(1)]);
        const p = s.commitStageAndWait([1], 60_000);
        s.handleMessage({ action: 'error', message: 'nope' });
        await expect(p).resolves.toBeUndefined();
    });

    it('resolves_after_the_timeout', async () => {
        vi.useFakeTimers();
        try {
            const s = storeWith([track(1)]);
            const p = s.commitStageAndWait([1], 5000);
            vi.advanceTimersByTime(5000);
            await expect(p).resolves.toBeUndefined();
        } finally { vi.useRealTimers(); }
    });
});

describe('journal', () => {
    const page = (ids: number[]) => ({
        action: 'journal', today: { committed: 1, tagsCreated: 0, activeSecs: 60 }, days: [], commits: [],
        trail: ids.map((id) => ({ id, at: 0, trackId: null, trackTitle: null, kind: 'staged', data: '{}' })),
    });

    it('load_more_appends_and_a_plain_refresh_replaces', () => {
        const s = new TagStore();
        s.requestJournal();
        s.handleMessage(page([5, 4]));
        s.requestJournal(4);
        expect(send).toHaveBeenLastCalledWith('journalGet', { beforeId: 4 });
        s.handleMessage(page([3, 2]));
        expect(s.journal.value?.trail.map((r) => r.id)).toEqual([5, 4, 3, 2]);
        s.requestJournal();
        s.handleMessage(page([6, 5]));
        expect(s.journal.value?.trail.map((r) => r.id)).toEqual([6, 5]);
    });
});

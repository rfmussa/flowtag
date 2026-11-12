import { describe, expect, it, vi } from 'vitest';
import { TagStore, FlowTrack, SavedFilter } from '@/scripts/tagstore';
vi.mock('@/scripts/onetagger', () => ({ get1t: () => ({ send: () => undefined }) }));

import { matchesStatus, passesQuickFilter, statusOf } from './status';

function track(over: Partial<FlowTrack> = {}): FlowTrack {
    return {
        id: 1, path: '/a.mp3', folderId: 1, format: 'mp3', title: 'T', artist: 'A',
        addedAt: 0, missing: false, needsFlush: false, tagIds: [], ...over,
    };
}

describe('statusOf', () => {
    it('status_precedence', () => {
        expect(statusOf({ doneAt: 5, staged: true, tagIds: [1] })).toBe('done');
        expect(statusOf({ doneAt: 5, tagIds: [] })).toBe('done');
        expect(statusOf({ doneAt: null, staged: true, tagIds: [1] })).toBe('staged');
        expect(statusOf({ staged: true, tagIds: [] })).toBe('staged');
        expect(statusOf({ doneAt: null, staged: false, tagIds: [1] })).toBe('inProgress');
        expect(statusOf({ tagIds: [] })).toBe('untagged');
    });
});

describe('quick filter', () => {
    it('quickfilter_notdone_hides_done_only', () => {
        const done = { doneAt: 9, tagIds: [1] };
        const staged = { staged: true, tagIds: [1] };
        const progress = { tagIds: [1] };
        const untagged = { tagIds: [] };
        const kept = (q: 'all' | 'notdone' | 'untagged') =>
            [done, staged, progress, untagged].map((t) => passesQuickFilter(t, q));
        expect(kept('all')).toEqual([true, true, true, true]);
        expect(kept('notdone')).toEqual([false, true, true, true]);
        expect(kept('untagged')).toEqual([false, false, false, true]);
    });
});

describe('smart status rule', () => {
    const store = new TagStore();
    const now = Date.now();
    const done = track({ id: 1, doneAt: 9, tagIds: [1] });
    const staged = track({ id: 2, staged: true, tagIds: [1] });
    const progress = track({ id: 3, tagIds: [1] });
    const untagged = track({ id: 4 });
    const ids = (f: SavedFilter) =>
        [done, staged, progress, untagged].filter((t) => store.passesSaved(t, f, now)).map((t) => t.id);

    it('smart_rule_status_is_and_is_not', () => {
        expect(ids({ status: { is: 'done' } })).toEqual([1]);
        expect(ids({ status: { is: 'inProgress' } })).toEqual([3]);
        expect(ids({ status: { is: 'notDone' } })).toEqual([2, 3, 4]);
        expect(ids({ status: { isNot: 'done' } })).toEqual([2, 3, 4]);
        expect(ids({ status: { isNot: 'notDone' } })).toEqual([1]);
        expect(ids({ status: { is: 'notDone', isNot: 'untagged' } })).toEqual([2, 3]);
        expect(ids({})).toEqual([1, 2, 3, 4]);
        expect(matchesStatus(done, { status: 'done', negate: true })).toBe(false);
    });

    it('legacy_filters_still_pass', () => {
        const legacy = JSON.parse('{"untaggedOnly":true,"tagIds":[1],"excludeTagIds":[2]}') as SavedFilter;
        // untaggedOnly + required tag can never hold: unchanged, regardless of status fields
        expect(ids(legacy)).toEqual([]);
        const tagsOnly = JSON.parse('{"tagIds":[1],"excludeTagIds":[2]}') as SavedFilter;
        expect(ids(tagsOnly)).toEqual([1, 2, 3]);
        expect(ids(JSON.parse('{"untaggedOnly":true}'))).toEqual([4]);
        const withExclude = track({ id: 5, doneAt: 3, staged: true, tagIds: [1, 2] });
        expect(store.passesSaved(withExclude, tagsOnly, now)).toBe(false);
    });
});

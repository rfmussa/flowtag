import { describe, expect, it, vi } from 'vitest';
vi.mock('@/scripts/onetagger', () => ({ get1t: () => ({ send: () => undefined }) }));

import { TagStore, FlowTrack } from '@/scripts/tagstore';
import { FlowTagSettings } from '@/scripts/settings';

/** Store-level rating behaviour: filter, sort, smart rule, echo, persistence */

function track(id: number, rating?: number | null): FlowTrack {
    return {
        id, path: `/m/${id}.mp3`, folderId: 1, format: 'mp3', title: `T${id}`, artist: 'A',
        addedAt: 0, missing: false, needsFlush: false, tagIds: [], rating,
    };
}

function storeWith(tracks: FlowTrack[]): TagStore {
    const s = new TagStore();
    s.tracks.value = tracks;
    return s;
}

const ids = (ts: FlowTrack[]) => ts.map((t) => t.id);

describe('min rating filter', () => {
    it('shows_tracks_with_at_least_n_stars_and_combines_with_quick_filter', () => {
        const s = storeWith([track(1, 5), track(2, 3), track(3, null), track(4, 4)]);
        expect(ids(s.visibleTracks())).toEqual([1, 2, 3, 4]);
        s.minRating.value = 4;
        expect(ids(s.visibleTracks())).toEqual([1, 4]);
        s.quickFilter.value = 'untagged';
        s.tracks.value = [...s.tracks.value, { ...track(5, 5), tagIds: [9] }];
        expect(ids(s.visibleTracks())).toEqual([1, 4]);
    });
});

describe('sort by rating', () => {
    it('orders_by_stars_with_unrated_last_in_both_directions', () => {
        const s = storeWith([track(1, 2), track(2, null), track(3, 5), track(4, 3)]);
        s.sortBy.value = 'rating';
        expect(ids(s.visibleTracks())).toEqual([1, 4, 3, 2]);
        s.sortDesc.value = true;
        expect(ids(s.visibleTracks())).toEqual([3, 4, 1, 2]);
    });
});

describe('smart playlist rating rule', () => {
    it('rating_min_rule_filters_and_bad_values_are_ignored', () => {
        const s = storeWith([]);
        const now = Date.now();
        expect(s.passesSaved(track(1, 4), { ratingMin: 4 }, now)).toBe(true);
        expect(s.passesSaved(track(1, 3), { ratingMin: 4 }, now)).toBe(false);
        expect(s.passesSaved(track(1, null), { ratingMin: 1 }, now)).toBe(false);
        expect(s.passesSaved(track(1, null), { ratingMin: 9 }, now)).toBe(true);
    });
});

describe('tracksRatingChanged echo', () => {
    it('applies_to_listed_ids_only_and_marks_them_pending', () => {
        const s = storeWith([track(1, 2), track(2, 2), track(3)]);
        const untouched = s.tracks.value[2];
        expect(s.handleMessage({ action: 'tracksRatingChanged', trackIds: [1, 2], rating: 5 })).toBe(true);
        expect(s.tracks.value.map((t) => [t.rating, t.needsFlush])).toEqual([[5, true], [5, true], [undefined, false]]);
        expect(s.tracks.value[2]).toBe(untouched);
        s.handleMessage({ action: 'tracksRatingChanged', trackIds: [1], rating: null });
        expect(s.tracks.value[0].rating).toBeNull();
    });

    it('an_invalid_rating_from_the_wire_clears_rather_than_storing_garbage', () => {
        const s = storeWith([track(1, 2)]);
        s.handleMessage({ action: 'tracksRatingChanged', trackIds: [1], rating: 42 });
        expect(s.tracks.value[0].rating).toBeNull();
    });
});

describe('persistence', () => {
    it('min_rating_survives_the_view_state_and_rejects_bad_values', () => {
        const view = (v: unknown) => FlowTagSettings.fromJson({ viewState: { minRating: v, sort: { by: 'rating' } } }).viewState;
        expect(view(3)?.minRating).toBe(3);
        expect(view(3)?.sort.by).toBe('rating');
        expect(view(7)?.minRating).toBe(0);
        expect(view('3')?.minRating).toBe(0);
    });

    it('rating_column_defaults_on_and_respects_a_saved_off', () => {
        expect(FlowTagSettings.fromJson({}).columns.rating).toBe(true);
        expect(FlowTagSettings.fromJson({ columns: { rating: false } }).columns.rating).toBe(false);
    });
});

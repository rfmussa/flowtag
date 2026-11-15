import { describe, expect, it } from 'vitest';

import {
    applyRating, isMinRating, isRatingBind, isRating, nextRating, passesMinRating, ratingForKey, ratingStars, shiftedDigit,
} from './rating';

function key(code: string, mods: Partial<KeyboardEvent> = {}) {
    return { code, shiftKey: false, ctrlKey: false, metaKey: false, altKey: false, ...mods };
}

describe('isRating / isMinRating', () => {
    it('accepts_only_integers_one_to_five', () => {
        for (const r of [1, 2, 3, 4, 5]) expect(isRating(r)).toBe(true);
        for (const bad of [0, 6, 2.5, -1, '3', null, undefined, NaN]) expect(isRating(bad)).toBe(false);
    });
    it('min_rating_adds_zero_as_off', () => {
        expect(isMinRating(0)).toBe(true);
        expect(isMinRating(5)).toBe(true);
        expect(isMinRating(6)).toBe(false);
        expect(isMinRating('2')).toBe(false);
    });
});

describe('ratingForKey', () => {
    it('shift_digit_one_to_five_maps_to_stars', () => {
        for (const d of [1, 2, 3, 4, 5]) expect(ratingForKey(key(`Digit${d}`, { shiftKey: true }))).toBe(d);
    });
    it('needs_shift_and_no_other_modifier', () => {
        expect(ratingForKey(key('Digit3'))).toBeUndefined();
        expect(ratingForKey(key('Digit3', { shiftKey: true, metaKey: true }))).toBeUndefined();
        expect(ratingForKey(key('Digit3', { shiftKey: true, ctrlKey: true }))).toBeUndefined();
        expect(ratingForKey(key('Digit3', { shiftKey: true, altKey: true }))).toBeUndefined();
    });
    it('shift_six_to_zero_and_letters_are_not_ratings', () => {
        for (const code of ['Digit6', 'Digit9', 'Digit0', 'KeyA']) {
            expect(ratingForKey(key(code, { shiftKey: true }))).toBeUndefined();
        }
    });
    it('shifted_digit_detects_every_shifted_digit', () => {
        expect(shiftedDigit(key('Digit7', { shiftKey: true }))).toBe(true);
        expect(shiftedDigit(key('Numpad2', { shiftKey: true }))).toBe(true);
        expect(shiftedDigit(key('Digit7'))).toBe(false);
        expect(shiftedDigit(key('KeyA', { shiftKey: true }))).toBe(false);
    });
});

describe('isRatingBind', () => {
    it('only_shift_one_to_five_without_other_modifiers', () => {
        expect(isRatingBind({ key: '3', shift: true })).toBe(true);
        expect(isRatingBind({ key: '3' })).toBe(false);
        expect(isRatingBind({ key: '6', shift: true })).toBe(false);
        expect(isRatingBind({ key: '3', shift: true, ctrl: true })).toBe(false);
        expect(isRatingBind({ key: 'a', shift: true })).toBe(false);
    });
});

describe('nextRating', () => {
    it('sets_the_pressed_rating', () => {
        expect(nextRating([undefined], 3)).toBe(3);
        expect(nextRating([5], 3)).toBe(3);
    });
    it('pressing_the_current_rating_clears', () => {
        expect(nextRating([3], 3)).toBeNull();
    });
    it('multi_clears_only_when_every_track_has_exactly_that_rating', () => {
        expect(nextRating([3, 3, 3], 3)).toBeNull();
        expect(nextRating([3, null, 3], 3)).toBe(3);
        expect(nextRating([3, 4], 3)).toBe(3);
    });
    it('empty_selection_sets', () => {
        expect(nextRating([], 2)).toBe(2);
    });
});

describe('passesMinRating', () => {
    it('zero_is_off', () => {
        expect(passesMinRating({}, 0)).toBe(true);
        expect(passesMinRating({ rating: null }, 0)).toBe(true);
    });
    it('at_least_n_stars', () => {
        expect(passesMinRating({ rating: 3 }, 3)).toBe(true);
        expect(passesMinRating({ rating: 5 }, 3)).toBe(true);
        expect(passesMinRating({ rating: 2 }, 3)).toBe(false);
        expect(passesMinRating({ rating: null }, 1)).toBe(false);
        expect(passesMinRating({}, 1)).toBe(false);
    });
});

describe('ratingStars', () => {
    it('filled_stars_only_and_empty_for_unrated', () => {
        expect(ratingStars(3)).toBe('★★★');
        expect(ratingStars(5)).toBe('★★★★★');
        expect(ratingStars(null)).toBe('');
        expect(ratingStars(undefined)).toBe('');
        expect(ratingStars(9)).toBe('');
    });
});

describe('applyRating', () => {
    const tracks = [
        { id: 1, rating: 2, needsFlush: false },
        { id: 2, rating: null, needsFlush: false },
        { id: 3, rating: 4, needsFlush: false },
    ];
    it('returns_new_objects_for_changed_tracks_only', () => {
        const out = applyRating(tracks, [1, 2], 4);
        expect(out).not.toBe(tracks);
        expect(out[0]).toEqual({ id: 1, rating: 4, needsFlush: true });
        expect(out[1]).toEqual({ id: 2, rating: 4, needsFlush: true });
        expect(out[2]).toBe(tracks[2]);
        expect(tracks[0].rating).toBe(2); // input untouched
    });
    it('no_op_tracks_keep_identity_and_flush_state', () => {
        const out = applyRating(tracks, [3], 4);
        expect(out[2]).toBe(tracks[2]);
    });
    it('null_clears', () => {
        expect(applyRating(tracks, [1], null)[0].rating).toBeNull();
    });
});

/** Star ratings (quality axis): 1–5 per track, null/absent = unrated.
 *  Pure helpers shared by the keymap, the store, the list row and the filters.
 *  The server validates again (crates/onetagger-db/src/ratings.rs). */

export const RATINGS = [1, 2, 3, 4, 5] as const;
export type Rating = typeof RATINGS[number];
/** The "≥ n stars" filter: 0 = off */
export type MinRating = 0 | Rating;

/** The rating-bearing slice of a track */
export interface RatingSource {
    rating?: number | null;
}

/** Key-event fields the rating keys look at */
export interface KeyLike {
    code: string;
    shiftKey: boolean;
    ctrlKey: boolean;
    metaKey: boolean;
    altKey: boolean;
}

const STAR = '★';

export function isRating(v: unknown): v is Rating {
    return typeof v === 'number' && (RATINGS as readonly number[]).includes(v);
}

export function isMinRating(v: unknown): v is MinRating {
    return v === 0 || isRating(v);
}

/** Shift+1…Shift+5 (top row, no other modifier) → that rating */
export function ratingForKey(e: KeyLike): Rating | undefined {
    if (!e.shiftKey || e.ctrlKey || e.metaKey || e.altKey) return undefined;
    const m = /^Digit([1-5])$/.exec(e.code);
    return m ? (Number(m[1]) as Rating) : undefined;
}

/** Is a stored keybind (Keybind shape: key '1', shift…) one of the rating chords?
 *  New tag hotkeys / action binds on ⇧1–⇧5 are refused for that reason. */
export function isRatingBind(kb: { key?: string; shift?: boolean; ctrl?: boolean; alt?: boolean }): boolean {
    return !!kb.shift && !kb.ctrl && !kb.alt && /^[1-5]$/.test(String(kb.key ?? ''));
}

/** Any shifted digit: the Shift+digit layer never seeks */
export function shiftedDigit(e: KeyLike): boolean {
    return e.shiftKey && /^(Digit|Numpad)\d$/.test(e.code);
}

/** The rating a press of `pressed` applies to the given current ratings:
 *  clears (null) only when every track already has exactly `pressed` */
export function nextRating(current: ReadonlyArray<number | null | undefined>, pressed: Rating): Rating | null {
    const allHaveIt = current.length > 0 && current.every((r) => r === pressed);
    return allHaveIt ? null : pressed;
}

/** "≥ n stars" filter; 0 lets everything through, unrated never passes n ≥ 1 */
export function passesMinRating(t: RatingSource, min: MinRating): boolean {
    if (min === 0) return true;
    return typeof t.rating === 'number' && t.rating >= min;
}

/** Filled stars only; unrated (or invalid) renders nothing */
export function ratingStars(rating: number | null | undefined): string {
    return isRating(rating) ? STAR.repeat(rating) : '';
}

/** New track array with `rating` applied to `ids`. Tracks that already carry
 *  it keep their identity (and flush state), as the server leaves them clean. */
export function applyRating<T extends RatingSource & { id: number; needsFlush: boolean }>(
    tracks: readonly T[], ids: Iterable<number>, rating: Rating | null,
): T[] {
    const targets = new Set(ids);
    return tracks.map((t) => {
        if (!targets.has(t.id) || (t.rating ?? null) === rating) return t;
        return { ...t, rating, needsFlush: true };
    });
}

/** Options for the "≥ n stars" filter select */
export const MIN_RATING_OPTIONS: ReadonlyArray<{ label: string; value: MinRating }> = [
    { label: 'Any rating', value: 0 },
    ...RATINGS.map((r) => ({ label: `≥ ${STAR.repeat(r)}`, value: r })),
];

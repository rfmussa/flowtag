/** Responsive layout helpers. Breakpoints (match the CSS media queries):
 *  full ≥1200 · medium 900–1199 · narrow 800–899 · resize gate below 800×450 */

export const MIN_WINDOW_W = 800;
export const MIN_WINDOW_H = 450;

/** How many bars the player waveform draws for a given window width,
 *  clamped so narrow windows never compute a negative count */
export function waveformWaves(windowWidth: number): number {
    return Math.min(Math.max(Math.round(20 + (windowWidth - 1200) / 10), 14), 70);
}

/** First `max` chips plus a "+n" count for the rest; no max shows everything */
export function capChips<T>(tags: readonly T[], max?: number): { shown: T[]; more: number } {
    if (max === undefined || tags.length <= max) return { shown: [...tags], more: 0 };
    return { shown: tags.slice(0, max), more: tags.length - max };
}

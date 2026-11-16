/** Skip flag helpers (X key, "Skip the rest" in the commit sheet).
 *  The server validates again (crates/onetagger-db/src/staging.rs `set_skip`). */
import type { StageRow } from '@/scripts/tagstore';
import type { Ticks } from '@/lib/commit';

/** The skip-bearing slice of a loaded track */
export interface SkipSource {
    readonly staged?: boolean;
    readonly skip?: boolean;
}

/** The skip value a press of X applies to the given tracks. Mirrors the stage
 *  toggle (S): if every track is already skipped it clears, otherwise it sets
 *  skip on all of them. An unstaged track counts as not skipped (X on it stages
 *  it and skips it, server-side). One track: a plain toggle. */
export function nextSkip(tracks: readonly SkipSource[]): boolean {
    const allSkipped = tracks.length > 0 && tracks.every((t) => !!t.staged && !!t.skip);
    return !allSkipped;
}

/** Ids of the stage rows that are not ready (no tags, not skipped), in stage order */
export function unreadyIds(rows: readonly StageRow[]): number[] {
    return rows.filter((r) => !r.ready).map((r) => r.trackId);
}

/** `ticks` with every given id ticked (new object) */
export function tickIds(ticks: Ticks, ids: readonly number[]): Ticks {
    return { ...ticks, ...Object.fromEntries(ids.map((id) => [id, true])) };
}

import type { StageRow } from '@/scripts/tagstore';

/** Which staged tracks are ticked for the commit, by track id */
export type Ticks = Readonly<Record<number, boolean>>;

export interface CommitPayload {
    readonly trackIds: number[];
    readonly message: string;
}

/** The loaded-track fields the sheet derives readiness from */
export interface ReadySource {
    readonly id: number;
    readonly tagIds: readonly number[];
    readonly skip?: boolean;
}

/** Stage rows (the server's list, in its order) with skip/ready re-derived from the
 *  loaded tracks (`skip || has tags`), so a stageState that predates the latest tag
 *  cannot leave a just-tagged track unticked. Tracks not loaded keep the server row. */
export function withLoadedReady(rows: readonly StageRow[], tracks: readonly ReadySource[]): StageRow[] {
    const byId = new Map(tracks.map((t) => [t.id, t]));
    return rows.map((r) => {
        const t = byId.get(r.trackId);
        if (!t) return r;
        const skip = !!t.skip;
        return { ...r, skip, ready: skip || t.tagIds.length > 0 };
    });
}

/** The Stage entry's count: "ready / total", flipping to "N left" once more than half is ready */
export function stageCountLabel(ready: number, total: number): string {
    return ready > total / 2 ? `${total - ready} left` : `${ready} / ${total}`;
}

/** Ready rows start ticked, unfinished ones do not */
export function initialTicks(rows: readonly StageRow[]): Ticks {
    return Object.fromEntries(rows.map((r) => [r.trackId, r.ready]));
}

export function toggleTick(ticks: Ticks, trackId: number): Ticks {
    return { ...ticks, [trackId]: !ticks[trackId] };
}

/** Ticked ids in stage order plus the trimmed message */
export function commitPayload(rows: readonly StageRow[], ticks: Ticks, message: string): CommitPayload {
    return {
        trackIds: rows.filter((r) => ticks[r.trackId]).map((r) => r.trackId),
        message: message.trim(),
    };
}

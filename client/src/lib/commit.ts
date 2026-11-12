import type { StageRow } from '@/scripts/tagstore';

/** Which staged tracks are ticked for the commit, by track id */
export type Ticks = Readonly<Record<number, boolean>>;

export interface CommitPayload {
    readonly trackIds: number[];
    readonly message: string;
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

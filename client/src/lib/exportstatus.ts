/** Rekordbox comment-export status transitions. Pure: each returns a new
 *  state; the store assigns it. */

export interface ExportState {
    running: boolean;
    done: number;
    total: number;
    /** Held after a finished export with failures, until dismissed */
    result?: { written: number; unchanged: number; failed: number };
}

export const IDLE_EXPORT: ExportState = { running: false, done: 0, total: 0 };

export interface ExportProgressMsg { done?: number; total?: number }
export interface ExportDoneMsg { result?: { written?: number; unchanged?: number; failed?: number } }

export function exportProgress(_state: ExportState, msg: ExportProgressMsg): ExportState {
    return { running: true, done: msg.done ?? 0, total: msg.total ?? 0 };
}

/** A failed write into real files must outlive the toast, so failures are
 *  held in the state until dismissed; a clean export returns to idle. */
export function exportDone(_state: ExportState, msg: ExportDoneMsg): ExportState {
    const r = msg.result ?? {};
    return (r.failed ?? 0) > 0
        ? { ...IDLE_EXPORT, result: { written: r.written ?? 0, unchanged: r.unchanged ?? 0, failed: r.failed ?? 0 } }
        : { ...IDLE_EXPORT };
}

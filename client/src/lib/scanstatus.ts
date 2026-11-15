/** Library-scan status transitions. Pure: each returns a new state. */

export interface ScanState {
    running: boolean;
    done: number;
    total: number;
    result?: { added: number; updated: number; removed: number; failed: number };
}

export const IDLE_SCAN: ScanState = { running: false, done: 0, total: 0 };

export function scanProgress(state: ScanState, msg: { done: number; total: number }): ScanState {
    return { ...state, running: true, done: msg.done, total: msg.total };
}

export function scanDone(_state: ScanState, result: ScanState['result']): ScanState {
    return { ...IDLE_SCAN, result };
}

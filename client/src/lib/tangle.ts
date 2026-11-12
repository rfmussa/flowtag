export type TangleState =
    | 'disabled' | 'noToken' | 'idle' | 'syncing' | 'tokenInvalid'
    | 'backupRequired' | 'waiting' | 'offline' | 'error';

/** Mirrors the backend `tangleStatus` event payload; times are unix seconds */
export interface TangleStatus {
    state: TangleState;
    user: string | null;
    matched: number;
    total: number;
    unmatchedPaths: string[];
    pending: number;
    lastSyncedAt: number | null;
    retryAt: number | null;
    /** [tagId, name] pairs of tags deleted in Tangle */
    unlinked: [number, string][];
    error: string | null;
}

function clock(unixSeconds: number): string {
    const d = new Date(unixSeconds * 1000);
    return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

function ago(lastSyncedAt: number, now: number): string {
    const secs = now - lastSyncedAt;
    if (secs < 60) return 'just now';
    if (secs < 3600) return `${Math.floor(secs / 60)} min ago`;
    return clock(lastSyncedAt);
}

/** Primary status line; unmatched tracks are a separate line (see unmatchedLine) */
export function statusLine(s: TangleStatus, now: number): string {
    switch (s.state) {
        case 'tokenInvalid': return 'Token expired. Paste a new one';
        case 'backupRequired': return 'Tangle needs Backup and restore on';
        case 'waiting': return s.retryAt != null ? `Waiting for Tangle until ${clock(s.retryAt)}` : 'Waiting for Tangle';
        case 'offline': return 'Offline, retrying';
        case 'error': return `Tangle error (${s.error ?? 'unknown'})`;
        case 'noToken': return 'Not connected';
        case 'disabled': return 'Sync is off';
    }
    if (s.pending > 0) return `Sending ${s.pending} ${s.pending === 1 ? 'change' : 'changes'}…`;
    const n = s.unlinked.length;
    if (n > 0) return n === 1 ? '1 tag was deleted in Tangle' : `${n} tags were deleted in Tangle`;
    if (s.lastSyncedAt == null) return 'Syncing…';
    return `Synced · ${ago(s.lastSyncedAt, now)}`;
}

export function unmatchedLine(s: TangleStatus): string {
    const n = s.unmatchedPaths.length;
    if (n === 0) return '';
    return n === 1 ? "1 track isn't in Tangle" : `${n} tracks aren't in Tangle`;
}

export function connectErrorText(code: string): string {
    switch (code) {
        case 'invalid_token': return 'Tangle rejected that token';
        case 'backup_and_restore_required': return 'Tangle needs Backup and restore on';
        case 'network': return "Couldn't reach Tangle";
        case 'empty_token': return 'Paste a token first';
        case 'unavailable': return "Tangle sync isn't available";
        default: return `Tangle error (${code})`;
    }
}

/** The first preview downloads the whole Tangle library, so give it time */
export const PREVIEW_TIMEOUT_MS = 120_000;

export function isConnected(s: TangleStatus | null): boolean {
    return !!s && !!s.user && s.state !== 'noToken';
}

/** The token field shows when disconnected, and when Tangle rejected the saved token */
export function needsToken(s: TangleStatus | null): boolean {
    return !isConnected(s) || s?.state === 'tokenInvalid';
}

import { describe, expect, it } from 'vitest';
import { connectErrorText, isConnected, needsToken, PREVIEW_TIMEOUT_MS, statusLine, unmatchedLine, TangleStatus } from './tangle';

const NOW = 1_800_000_000;

function status(over: Partial<TangleStatus> = {}): TangleStatus {
    return {
        state: 'idle', user: 'dj', matched: 10, total: 10, unmatchedPaths: [], pending: 0,
        lastSyncedAt: NOW - 120, retryAt: null, unlinked: [], error: null, ...over,
    };
}

function localSeconds(h: number, m: number): number {
    return new Date(2026, 10, 12, h, m, 30).getTime() / 1000;
}

describe('statusLine', () => {
    it('shows time since the last sync', () => {
        expect(statusLine(status(), NOW)).toBe('Synced · 2 min ago');
        expect(statusLine(status({ lastSyncedAt: NOW - 10 }), NOW)).toBe('Synced · just now');
    });
    it('falls back to a clock time after an hour', () => {
        const at = localSeconds(9, 5);
        expect(statusLine(status({ lastSyncedAt: at }), at + 7200)).toBe('Synced · 09:05');
    });
    it('says Syncing… when nothing has synced yet', () => {
        expect(statusLine(status({ lastSyncedAt: null }), NOW)).toBe('Syncing…');
        expect(statusLine(status({ state: 'syncing', lastSyncedAt: null }), NOW)).toBe('Syncing…');
    });
    it('shows pending changes with singular form', () => {
        expect(statusLine(status({ state: 'syncing', pending: 340 }), NOW)).toBe('Sending 340 changes…');
        expect(statusLine(status({ pending: 1 }), NOW)).toBe('Sending 1 change…');
    });
    it('reports unlinked tags', () => {
        expect(statusLine(status({ unlinked: [[1, 'a']] }), NOW)).toBe('1 tag was deleted in Tangle');
        expect(statusLine(status({ unlinked: [[1, 'a'], [2, 'b']] }), NOW)).toBe('2 tags were deleted in Tangle');
    });
    it('reports problem states ahead of everything else', () => {
        expect(statusLine(status({ state: 'tokenInvalid', pending: 5 }), NOW)).toBe('Token expired. Paste a new one');
        expect(statusLine(status({ state: 'backupRequired' }), NOW)).toBe('Tangle needs Backup and restore on');
        expect(statusLine(status({ state: 'waiting', retryAt: localSeconds(14, 5) }), NOW)).toBe('Waiting for Tangle until 14:05');
        expect(statusLine(status({ state: 'offline' }), NOW)).toBe('Offline, retrying');
        expect(statusLine(status({ state: 'error', error: 'server' }), NOW)).toBe('Tangle error (server)');
    });
    it('reports not connected and off', () => {
        expect(statusLine(status({ state: 'noToken', user: null }), NOW)).toBe('Not connected');
        expect(statusLine(status({ state: 'disabled' }), NOW)).toBe('Sync is off');
    });
});

describe('unmatchedLine', () => {
    it('uses singular and plural copy, empty for zero', () => {
        expect(unmatchedLine(status({ unmatchedPaths: ['a'] }))).toBe("1 track isn't in Tangle");
        expect(unmatchedLine(status({ unmatchedPaths: ['a', 'b', 'c', 'd', 'e', 'f'] }))).toBe("6 tracks aren't in Tangle");
        expect(unmatchedLine(status())).toBe('');
    });
});

describe('connectErrorText', () => {
    it('maps known codes', () => {
        expect(connectErrorText('invalid_token')).toBe('Tangle rejected that token');
        expect(connectErrorText('backup_and_restore_required')).toBe('Tangle needs Backup and restore on');
        expect(connectErrorText('network')).toBe("Couldn't reach Tangle");
        expect(connectErrorText('empty_token')).toBe('Paste a token first');
        expect(connectErrorText('unavailable')).toBe("Tangle sync isn't available");
    });
    it('falls back to the code', () => {
        expect(connectErrorText('rate_limited')).toBe('Tangle error (rate_limited)');
    });
});

describe('connection state', () => {
    it('is connected when a user is present and a token exists', () => {
        expect(isConnected(status())).toBe(true);
        expect(isConnected(status({ state: 'noToken', user: null }))).toBe(false);
        expect(isConnected(null)).toBe(false);
    });
    it('asks for a token when not connected or when the token expired', () => {
        expect(needsToken(null)).toBe(true);
        expect(needsToken(status({ state: 'noToken', user: null }))).toBe(true);
        expect(needsToken(status({ state: 'tokenInvalid' }))).toBe(true);
        expect(needsToken(status())).toBe(false);
    });
    it('waits long enough for the first preview download', () => {
        expect(PREVIEW_TIMEOUT_MS).toBe(120_000);
    });
});

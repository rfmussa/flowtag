import { describe, expect, it } from 'vitest';
import { welcomeText, welcomeKey, shouldShowWelcome, resolveViewState } from '@/lib/welcome';
import type { ViewState } from '@/scripts/settings';

const secs = (y: number, mo: number, d: number, h: number, mi: number) => new Date(y, mo, d, h, mi).getTime() / 1000;
const now = new Date(2026, 10, 12, 9, 0).getTime();

const view = (over: Partial<ViewState> = {}): ViewState => ({
    scope: 'all', filterText: '', quickFilter: 'all', sort: { by: 'path', desc: false },
    selectedTrackId: null, firstVisibleIndex: 0, ...over,
});

describe('welcomeText', () => {
    it('welcome_text_quotes_commit_message_and_counts', () => {
        const commit = { id: 4, createdAt: secs(2026, 10, 10, 23, 41), message: 'Friday warmup', trackCount: 27 };
        // 10 Nov 2026 is a Tuesday, two days before "now"
        expect(welcomeText(commit, 12, now)).toBe("Tuesday 23:41 · committed 27 · 'Friday warmup'\nStage: 12 left");
    });
    it('omits the quote for an empty message and the stage line at zero', () => {
        const commit = { id: 4, createdAt: secs(2026, 10, 12, 1, 5), message: '  ', trackCount: 1 };
        expect(welcomeText(commit, 0, now)).toBe('Today 01:05 · committed 1');
    });
    it('uses Yesterday inside a day and a date after a week', () => {
        const y = { id: 1, createdAt: secs(2026, 10, 11, 20, 0), message: '', trackCount: 3 };
        expect(welcomeText(y, 0, now)).toBe('Yesterday 20:00 · committed 3');
        const old = { id: 1, createdAt: secs(2026, 9, 20, 8, 30), message: '', trackCount: 3 };
        expect(welcomeText(old, 0, now)).toBe('20 Oct 08:30 · committed 3');
    });
    it('shows only the stage line without a commit, nothing without either', () => {
        expect(welcomeText(null, 5, now)).toBe('Stage: 5 left');
        expect(welcomeText(null, 0, now)).toBeNull();
    });
});

describe('welcome visibility', () => {
    it('welcome_hidden_when_seen_key_matches', () => {
        const commit = { id: 4, createdAt: secs(2026, 10, 12, 1, 5), message: '', trackCount: 1 };
        const text = welcomeText(commit, 2, now);
        expect(welcomeKey(4, 2)).toBe('4:2');
        expect(welcomeKey(undefined, 0)).toBe('none:0');
        expect(shouldShowWelcome(text, undefined, '4:2')).toBe(true);
        expect(shouldShowWelcome(text, '4:2', '4:2')).toBe(false);
        expect(shouldShowWelcome(text, '4:1', '4:2')).toBe(true);
        expect(shouldShowWelcome(null, undefined, 'none:0')).toBe(false);
    });
});

describe('resolveViewState', () => {
    const ctx = { playlistIds: [1, 2], stageTotal: 3, trackIds: [10, 11, 12] };
    it('viewstate_fallback_on_missing_playlist', () => {
        expect(resolveViewState(view({ scope: 9 }), ctx).scope).toBe('all');
        expect(resolveViewState(view({ scope: 2 }), ctx).scope).toBe(2);
    });
    it('falls back from an empty stage', () => {
        expect(resolveViewState(view({ scope: 'stage' }), { ...ctx, stageTotal: 0 }).scope).toBe('all');
        expect(resolveViewState(view({ scope: 'stage' }), ctx).scope).toBe('stage');
    });
    it('drops a missing selection but keeps the scope', () => {
        const r = resolveViewState(view({ scope: 2, selectedTrackId: 99 }), ctx);
        expect(r.scope).toBe(2);
        expect(r.selectedTrackId).toBeNull();
        expect(resolveViewState(view({ selectedTrackId: 11 }), ctx).selectedTrackId).toBe(11);
    });
});

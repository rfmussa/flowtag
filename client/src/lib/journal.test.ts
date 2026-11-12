import { describe, expect, it } from 'vitest';
import { chartBars, formatActive, groupTrail, trailLine, TrailRow } from '@/lib/journal';

const row = (kind: string, data: unknown = {}, extra: Partial<TrailRow> = {}): TrailRow => ({
    id: 1, at: 0, trackId: 7, trackTitle: 'Tonight', kind,
    data: typeof data === 'string' ? data : JSON.stringify(data), ...extra,
});

describe('journal', () => {
    it('active_time_formats', () => {
        expect(formatActive(4320)).toBe('1h 12m');
        expect(formatActive(720)).toBe('12m');
        expect(formatActive(3600)).toBe('1h 0m');
        expect(formatActive(30)).toBe('<1m');
        expect(formatActive(0)).toBe('0m');
    });

    it('trail_lines_and_inverses', () => {
        expect(trailLine(row('tag_added', { tagId: 3, tagName: 'dark' }))).toEqual({
            text: '+dark · Tonight', undo: { action: 'trackRemoveTag', data: { trackId: 7, tagId: 3 } },
        });
        expect(trailLine(row('tag_removed', { tagId: 3, tagName: 'dark' }))).toEqual({
            text: '−dark · Tonight', undo: { action: 'trackAddTag', data: { trackId: 7, name: 'dark' } },
        });
        expect(trailLine(row('skip_set'))).toEqual({ text: 'skip · Tonight', undo: { action: 'stageSkip', data: { trackId: 7 } } });
        expect(trailLine(row('staged'))).toEqual({
            text: 'staged · Tonight', undo: { action: 'unstageTracks', data: { trackIds: [7] } },
        });
        expect(trailLine(row('skip_cleared'))).toEqual({ text: 'skip cleared · Tonight' });
        expect(trailLine(row('unstaged'))).toEqual({ text: 'unstaged · Tonight' });
        expect(trailLine(row('committed', { commitId: 4 }))).toEqual({ text: 'committed · Tonight' });
        expect(trailLine(row('committed', { commitId: 4 }), [{ id: 4, createdAt: 0, message: 'late set', trackCount: 27 }]))
            .toEqual({ text: 'Committed 27 tracks · "late set"' });
        expect(trailLine(row('commit_reverted', { commitId: 4, count: 3 }, { trackId: null, trackTitle: null })))
            .toEqual({ text: 'Commit reverted · 3 tracks back on the stage' });
        expect(trailLine(row('tag_created', { tagId: 1, tagName: 'late-night' }, { trackId: null, trackTitle: null })))
            .toEqual({ text: 'Tag created: late-night' });
        expect(trailLine(row('tag_renamed', { tagId: 1, from: 'dark', to: 'darker' }, { trackId: null, trackTitle: null })))
            .toEqual({ text: 'Tag renamed: dark → darker' });
        expect(trailLine(row('tag_deleted', { tagId: 1, tagName: 'dark' }, { trackId: null, trackTitle: null })))
            .toEqual({ text: 'Tag deleted: dark' });
    });

    it('trail_lines_tolerate_junk', () => {
        expect(trailLine(row('tag_added', 'not json')).undo).toBeUndefined();
        expect(trailLine(row('tag_added', 'not json')).text).toBe('+? · Tonight');
        expect(trailLine(row('mystery', {}, { trackTitle: null })).text).toBe('mystery');
    });

    it('chart_scales_to_max_day', () => {
        expect(chartBars([['2026-11-10', 5], ['2026-11-11', 10], ['2026-11-12', 0]]))
            .toEqual([0.5, 1, 0]);
        expect(chartBars([['2026-11-10', 0]])).toEqual([0]);
        expect(chartBars([])).toEqual([]);
        expect(chartBars([['2026-11-10', 5]], 20)).toEqual([0.25]);
    });
});

describe('groupTrail', () => {
    const dayOf = (secs: number) => (secs < 1000 ? 'Mon' : 'Tue');
    const commits = [{ id: 4, createdAt: 0, message: 'late set', trackCount: 27 }];

    it('trail_collapses_one_commit_into_one_line', () => {
        const trail = Array.from({ length: 27 }, (_, i) =>
            row('committed', { commitId: 4 }, { id: 100 - i, at: 10, trackId: i, trackTitle: `T${i}` }));
        const groups = groupTrail(trail, commits, dayOf);
        expect(groups).toHaveLength(1);
        expect(groups[0].lines).toHaveLength(1);
        expect(groups[0].lines[0]).toMatchObject({ id: 100, at: 10, line: { text: 'Committed 27 tracks · "late set"' } });
    });

    it('keeps other rows and separate commits apart, grouped by day', () => {
        const trail = [
            row('committed', { commitId: 5 }, { id: 9, at: 2000 }),
            row('committed', { commitId: 5 }, { id: 8, at: 2000 }),
            row('tag_added', { tagId: 3, tagName: 'dark' }, { id: 7, at: 1500 }),
            row('committed', { commitId: 4 }, { id: 6, at: 10 }),
            row('committed', { commitId: 4 }, { id: 5, at: 10 }),
            row('committed', { commitId: 3 }, { id: 4, at: 10 }),
        ];
        const groups = groupTrail(trail, commits, dayOf);
        expect(groups.map((g) => g.day)).toEqual(['Tue', 'Mon']);
        expect(groups[0].lines.map((l) => l.line.text)).toEqual(['Committed 2 tracks', '+dark · Tonight']);
        expect(groups[1].lines.map((l) => l.line.text)).toEqual(['Committed 2 tracks · "late set"', 'committed · Tonight']);
    });
});

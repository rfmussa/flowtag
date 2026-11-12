/** Pure helpers for the Journal dialog: time formatting, trail lines and the chart */

export interface TrailRow {
    id: number;
    at: number;
    trackId: number | null;
    trackTitle: string | null;
    kind: string;
    /** JSON string */
    data: string;
}

export interface CommitRow {
    id: number;
    createdAt: number;
    message: string;
    trackCount: number;
}

export interface JournalState {
    today: { committed: number; tagsCreated: number; activeSecs: number };
    days: [string, number][];
    trail: TrailRow[];
    commits: CommitRow[];
}

export interface TrailLine {
    text: string;
    undo?: { action: string; data: any };
}

/** "1h 12m", "12m", "<1m" under a minute, "0m" for none */
export function formatActive(secs: number): string {
    if (secs <= 0) return '0m';
    if (secs < 60) return '<1m';
    const mins = Math.floor(secs / 60);
    const h = Math.floor(mins / 60);
    return h > 0 ? `${h}h ${mins % 60}m` : `${mins}m`;
}

function parseData(raw: string): Record<string, any> {
    try {
        const v = JSON.parse(raw);
        return v && typeof v === 'object' ? v : {};
    } catch {
        return {};
    }
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

/** One human line per trail row, plus the op that reverses it when there is one.
 *  `commits` lets a per-track committed row name its commit. */
export function trailLine(row: TrailRow, commits: CommitRow[] = []): TrailLine {
    const d = parseData(row.data);
    const title = row.trackTitle ?? '';
    const tag = typeof d.tagName === 'string' ? d.tagName : undefined;
    const withTitle = (head: string) => (title ? `${head} · ${title}` : head);
    switch (row.kind) {
        case 'tag_added':
            return {
                text: withTitle(`+${tag ?? '?'}`),
                ...(row.trackId !== null && typeof d.tagId === 'number'
                    ? { undo: { action: 'trackRemoveTag', data: { trackId: row.trackId, tagId: d.tagId } } } : {}),
            };
        case 'tag_removed':
            return {
                text: withTitle(`−${tag ?? '?'}`),
                ...(row.trackId !== null && tag !== undefined
                    ? { undo: { action: 'trackAddTag', data: { trackId: row.trackId, name: tag } } } : {}),
            };
        case 'skip_set':
            return {
                text: withTitle('skip'),
                ...(row.trackId !== null ? { undo: { action: 'stageSkip', data: { trackId: row.trackId } } } : {}),
            };
        case 'skip_cleared':
            return { text: withTitle('skip cleared') };
        case 'staged':
            return {
                text: withTitle('staged'),
                ...(row.trackId !== null ? { undo: { action: 'unstageTracks', data: { trackIds: [row.trackId] } } } : {}),
            };
        case 'unstaged':
            return { text: withTitle('unstaged') };
        case 'committed': {
            const commit = commits.find((c) => c.id === d.commitId);
            if (!commit) return { text: withTitle('committed') };
            return { text: `Committed ${plural(commit.trackCount, 'track')}${commit.message ? ` · "${commit.message}"` : ''}` };
        }
        case 'commit_reverted':
            return { text: typeof d.count === 'number'
                ? `Commit reverted · ${plural(d.count, 'track')} back on the stage` : 'Commit reverted' };
        case 'tag_created':
            return { text: `Tag created: ${tag ?? '?'}` };
        case 'tag_renamed':
            return { text: `Tag renamed: ${d.from ?? '?'} → ${d.to ?? '?'}` };
        case 'tag_deleted':
            return { text: `Tag deleted: ${tag ?? '?'}` };
        default:
            return { text: withTitle(row.kind) };
    }
}

/** Bar heights 0–1, relative to the busiest day (or `max` when given) */
export function chartBars(days: [string, number][], max?: number): number[] {
    const top = max ?? Math.max(0, ...days.map(([, n]) => n));
    return days.map(([, n]) => (top > 0 ? n / top : 0));
}

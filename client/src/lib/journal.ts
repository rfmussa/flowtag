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

export interface TrailEntry {
    readonly id: number;
    readonly at: number;
    readonly line: TrailLine;
}

export interface DayGroup {
    readonly day: string;
    readonly lines: readonly TrailEntry[];
}

const commitIdOf = (row: TrailRow): unknown => (row.kind === 'committed' ? parseData(row.data).commitId : undefined);

/** One entry per trail row, except a run of consecutive per-track "committed" rows
 *  of one commit, which becomes a single "Committed N tracks" line (N = the run) */
function trailEntries(trail: readonly TrailRow[], commits: CommitRow[]): TrailEntry[] {
    const out: TrailEntry[] = [];
    let i = 0;
    while (i < trail.length) {
        const row = trail[i];
        const commitId = commitIdOf(row);
        let end = i + 1;
        while (commitId !== undefined && end < trail.length && commitIdOf(trail[end]) === commitId) end++;
        const n = end - i;
        if (n === 1) {
            out.push({ id: row.id, at: row.at, line: trailLine(row, commits) });
        } else {
            const commit = commits.find((c) => c.id === commitId);
            const quote = commit?.message ? ` · "${commit.message}"` : '';
            out.push({ id: row.id, at: row.at, line: { text: `Committed ${plural(n, 'track')}${quote}` } });
        }
        i = end;
    }
    return out;
}

/** Trail rows (newest first) as day groups; consecutive rows of one day stay together */
export function groupTrail(trail: readonly TrailRow[], commits: CommitRow[], dayOf: (secs: number) => string): DayGroup[] {
    const out: DayGroup[] = [];
    for (const entry of trailEntries(trail, commits)) {
        const day = dayOf(entry.at);
        const last = out[out.length - 1];
        if (last && last.day === day) out[out.length - 1] = { day, lines: [...last.lines, entry] };
        else out.push({ day, lines: [entry] });
    }
    return out;
}

/** Bar heights 0–1, relative to the busiest day (or `max` when given) */
export function chartBars(days: [string, number][], max?: number): number[] {
    const top = max ?? Math.max(0, ...days.map(([, n]) => n));
    return days.map(([, n]) => (top > 0 ? n / top : 0));
}

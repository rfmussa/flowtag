import type { CommitRow } from '@/lib/journal';
import type { ViewState } from '@/scripts/settings';

const WEEKDAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const DAY_MS = 86_400_000;

const pad = (n: number) => String(n).padStart(2, '0');
const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

/** "Today 01:05", "Yesterday 20:00", "Tuesday 23:41" (under a week), "20 Oct 08:30" */
function whenLabel(createdAtSecs: number, now: number): string {
    const at = new Date(createdAtSecs * 1000);
    const time = `${pad(at.getHours())}:${pad(at.getMinutes())}`;
    const daysAgo = Math.round((startOfDay(new Date(now)) - startOfDay(at)) / DAY_MS);
    if (daysAgo <= 0) return `Today ${time}`;
    if (daysAgo === 1) return `Yesterday ${time}`;
    if (daysAgo < 7) return `${WEEKDAYS[at.getDay()]} ${time}`;
    return `${at.getDate()} ${MONTHS[at.getMonth()]} ${time}`;
}

/** Text of the welcome-back card (lines joined by \n), or null when there is nothing to show */
export function welcomeText(lastCommit: CommitRow | null, stageLeft: number, now: number): string | null {
    const lines: string[] = [];
    if (lastCommit) {
        const message = lastCommit.message.trim();
        const quote = message ? ` · '${message}'` : '';
        lines.push(`${whenLabel(lastCommit.createdAt, now)} · committed ${lastCommit.trackCount}${quote}`);
    }
    if (stageLeft > 0) lines.push(`Stage: ${stageLeft} left`);
    return lines.length > 0 ? lines.join('\n') : null;
}

export function welcomeKey(lastCommitId: number | undefined, stageLeft: number): string {
    return `${lastCommitId ?? 'none'}:${stageLeft}`;
}

export function shouldShowWelcome(text: string | null, seen: string | undefined, key: string): boolean {
    return text !== null && seen !== key;
}

export interface RestoreContext {
    playlistIds: readonly number[];
    stageTotal: number;
    trackIds: readonly number[];
}

/** Drop what no longer exists: a gone playlist or empty stage falls back to 'all', a gone track loses the selection */
export function resolveViewState(view: ViewState, ctx: RestoreContext): ViewState {
    const scopeGone = (typeof view.scope === 'number' && !ctx.playlistIds.includes(view.scope))
        || (view.scope === 'stage' && ctx.stageTotal === 0);
    const trackGone = view.selectedTrackId !== null && !ctx.trackIds.includes(view.selectedTrackId);
    return {
        ...view,
        scope: scopeGone ? 'all' : view.scope,
        selectedTrackId: trackGone ? null : view.selectedTrackId,
    };
}

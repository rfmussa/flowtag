/** Track status: one definition used by the list, quick filter and smart rules.
 *  Precedence: done (done_at set) -> staged -> in progress (has tags) -> untagged. */
export type TrackStatus = 'done' | 'staged' | 'inProgress' | 'untagged';
export type StatusValue = TrackStatus | 'notDone';
export type QuickFilter = 'all' | 'notdone' | 'untagged';

export interface StatusSource {
    doneAt?: number | null;
    staged?: boolean;
    tagIds: number[];
}

const STATUS_VALUES: readonly StatusValue[] = ['done', 'staged', 'inProgress', 'untagged', 'notDone'];

export function isStatusValue(v: unknown): v is StatusValue {
    return typeof v === 'string' && (STATUS_VALUES as readonly string[]).includes(v);
}

export function statusOf(t: StatusSource): TrackStatus {
    if (t.doneAt !== undefined && t.doneAt !== null) return 'done';
    if (t.staged) return 'staged';
    return t.tagIds.length > 0 ? 'inProgress' : 'untagged';
}

export function matchesStatus(t: StatusSource, rule: { status: StatusValue; negate?: boolean }): boolean {
    const s = statusOf(t);
    const hit = rule.status === 'notDone' ? s !== 'done' : s === rule.status;
    return rule.negate ? !hit : hit;
}

/** The header quick filter: notdone hides only done tracks; untagged shows only untagged */
export function passesQuickFilter(t: StatusSource, q: QuickFilter): boolean {
    if (q === 'notdone') return statusOf(t) !== 'done';
    if (q === 'untagged') return statusOf(t) === 'untagged';
    return true;
}

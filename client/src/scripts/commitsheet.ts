/** Opens the stage commit sheet. Placeholder: the sheet itself arrives with the
 *  commit-sheet task, which fills this in. */
export function openCommitSheet(): void {
    // intentionally empty until the commit sheet exists
}

/** ⌘Enter / Ctrl+Enter: opens the commit sheet even from a focused input */
export function isCommitChord(event: KeyboardEvent): boolean {
    return event.code === 'Enter' && (event.metaKey || event.ctrlKey) && !event.shiftKey && !event.altKey;
}

/** Does the tag input keep this key to itself? The commit chord passes through
 *  untouched (no add-tag, no stopPropagation) so the global handler sees it. */
export function tagInputOwnsKey(event: KeyboardEvent): boolean {
    return !isCommitChord(event);
}

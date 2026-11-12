/** Opens the stage commit sheet. Placeholder: the sheet itself arrives with the
 *  commit-sheet task, which fills this in. */
export function openCommitSheet(): void {
    // intentionally empty until the commit sheet exists
}

/** ⌘Enter / Ctrl+Enter: opens the commit sheet even from a focused input */
export function isCommitChord(event: KeyboardEvent): boolean {
    return event.code === 'Enter' && (event.metaKey || event.ctrlKey) && !event.shiftKey && !event.altKey;
}

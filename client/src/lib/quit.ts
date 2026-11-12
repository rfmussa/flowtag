export interface QuitDeps {
    readonly stageReady: number;
    readonly readyIds: number[];
    /** A tagging run holds the app: the existing guard owns the exit */
    readonly taggingRunning: boolean;
    /** ok = Commit & quit; cancel = Quit anyway; dismiss = stay in the app */
    readonly confirm: () => Promise<'ok' | 'cancel' | 'dismiss'>;
    /** Commit the ids and wait (bounded) for the server */
    readonly commitAndWait: (ids: number[]) => Promise<void>;
    readonly forceExit: () => void;
    readonly exitNormally: () => void;
}

/** Offer to commit ready staged tracks before quitting. Once the user chose
 *  "Commit & quit" the app always quits, whatever the commit does; "Quit
 *  anyway" takes the normal exit; dismissing the prompt aborts the quit. */
export async function runQuitFlow(d: QuitDeps): Promise<void> {
    if (d.taggingRunning || d.stageReady <= 0 || d.readyIds.length === 0) {
        d.exitNormally();
        return;
    }
    const choice = await d.confirm();
    if (choice === 'dismiss') return;
    if (choice === 'cancel') {
        d.exitNormally();
        return;
    }
    try {
        await d.commitAndWait(d.readyIds);
    } catch {
        // the commit failed or the socket is gone: quit regardless
    } finally {
        d.forceExit();
    }
}

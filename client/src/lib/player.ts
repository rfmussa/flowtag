/** Whether the selected track is worth preloading: something is selected and it is not already the loaded track */
export function shouldPreload(selPath: string | null, playingPath: string | null): boolean {
    return selPath !== null && selPath !== playingPath;
}

export interface TrackEndContext {
    autoPlayNext: boolean;
    onQuickTag: boolean;
    multiSelect: boolean;
}

export interface EndOfTrackState extends TrackEndContext {
    playing: boolean;
    position: number;
    duration: number;
    /** Clock slack before acting; the server's exact end event acts first */
    graceMs?: number;
}

/** What to do when a track has actually ended: advance on QuickTag without a
 *  multi-selection (advancing would silently collapse it), else pause */
export function trackEndAction(c: TrackEndContext): 'advance' | 'pause' {
    return (c.autoPlayNext && c.onQuickTag && !c.multiSelect) ? 'advance' : 'pause';
}

/** The clock's grace past a stated duration: estimates on headerless VBR files
 *  run short, so the exact server end event gets this long to act first */
export const CLOCK_END_GRACE_MS = 30_000;

/** What the player clock should do this tick. Only a PLAYING track past its
 *  duration (plus grace) acts — a paused one must not re-pause every tick. */
export function endOfTrackAction(s: EndOfTrackState): 'advance' | 'pause' | 'none' {
    if (!s.playing || s.position <= s.duration + (s.graceMs ?? 0)) return 'none';
    return trackEndAction(s);
}

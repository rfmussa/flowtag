/** Whether the selected track is worth preloading: something is selected and it is not already the loaded track */
export function shouldPreload(selPath: string | null, playingPath: string | null): boolean {
    return selPath !== null && selPath !== playingPath;
}

export interface EndOfTrackState {
    playing: boolean;
    position: number;
    duration: number;
    autoPlayNext: boolean;
    onQuickTag: boolean;
    multiSelect: boolean;
}

/** What the player clock should do this tick. Only a PLAYING track past its
 *  duration acts (a paused one must not re-pause every tick), and advancing
 *  is reserved for QuickTag without a multi-selection (advancing would
 *  silently collapse it). */
export function endOfTrackAction(s: EndOfTrackState): 'advance' | 'pause' | 'none' {
    if (!s.playing || s.position <= s.duration) return 'none';
    return (s.autoPlayNext && s.onQuickTag && !s.multiSelect) ? 'advance' : 'pause';
}

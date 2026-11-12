/** Whether the selected track is worth preloading: something is selected and it is not already the loaded track */
export function shouldPreload(selPath: string | null, playingPath: string | null): boolean {
    return selPath !== null && selPath !== playingPath;
}

/**
 * Blur the activated control after MOUSE activation only, so a later Space
 * press can't re-trigger it — while keyboard activation keeps its visible
 * focus trail intact (a keyboard "click" arrives as a MouseEvent with
 * detail 0, a real pointer click with detail ≥ 1).
 */
export function blurIfMouse(e: Event): void {
    if ((e as MouseEvent).detail === 0) return;
    (e.currentTarget as HTMLElement | null)?.blur();
}

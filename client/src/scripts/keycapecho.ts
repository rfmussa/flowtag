/** Keyboard-first made visible: when a key fires outside a text field, every
 *  visible on-screen keycap hint (.ft-kbd) showing that key dips for a beat.
 *  Purely presentational; never touches the event. */

/** How keycaps label non-letter keys, mapped from KeyboardEvent.key */
const KEY_LABELS: Record<string, string> = {
    ' ': 'SPACE',
    'ESCAPE': 'ESC',
    'ENTER': 'ENTER',
    'ARROWUP': '↑',
    'ARROWDOWN': '↓',
    'ARROWLEFT': '←',
    'ARROWRIGHT': '→',
};

function isTextTarget(target: EventTarget | null): boolean {
    const el = target as HTMLElement | null;
    if (!el || !el.tagName) return false;
    return el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable;
}

export function installKeycapEcho() {
    document.addEventListener('keydown', (event) => {
        if (event.metaKey || event.ctrlKey || event.altKey) return;
        if (isTextTarget(event.target)) return;
        const raw = event.key.toUpperCase();
        const label = KEY_LABELS[raw] ?? raw;
        if (label.length > 5) return;
        document.querySelectorAll('.ft-kbd').forEach((cap) => {
            if (cap.textContent?.trim().toUpperCase() !== label) return;
            cap.classList.add('kbd-echo');
            window.setTimeout(() => cap.classList.remove('kbd-echo'), 110);
        });
    }, { capture: true, passive: true });
}

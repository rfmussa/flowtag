import { get1t } from './onetagger';
import { getTagStore } from './tagstore';
import { Keybind } from './utils';

/**
 * Keyboard map for the FlowTag (Quick Tag) view.
 *
 * Called from the global keydown handler when the route is /quicktag and the
 * event target is not an input. Returns true when the key was handled (the
 * caller then prevents default).
 *
 * Transport:  Space play/pause · J/↑ prev · K/↓ next (vim-style) ·
 *             ←/→ seek -10s/+30s · 1/2/3 seek 25/50/75% · 0 seek start
 * Tagging:    T or Tab focus the tag input · D save + jump to next untagged ·
 *             per-tag hotkeys toggle their tag on the selected track
 */
export function handleFlowKeys(event: KeyboardEvent): boolean {
    const $1t = get1t();
    const store = getTagStore();
    const player = $1t.player.value;

    // Transport controls work with or without a selection
    switch (event.code) {
        case 'Space':
            if (player.playing) player.pause();
            else player.play();
            return true;
        case 'ArrowUp':
        case 'KeyJ':
            store.selectOffset(-1);
            return true;
        case 'ArrowDown':
        case 'KeyK':
            store.selectOffset(1);
            return true;
        case 'ArrowLeft':
            player.seek(Math.max(player.position - 10_000, 0));
            return true;
        case 'ArrowRight':
            player.seek(Math.min(player.position + 30_000, player.duration));
            return true;
        case 'Digit0':
        case 'Numpad0':
            player.seek(0);
            return true;
        case 'Digit1':
        case 'Numpad1':
            player.seek(Math.round(player.duration * 0.25));
            return true;
        case 'Digit2':
        case 'Numpad2':
            player.seek(Math.round(player.duration * 0.5));
            return true;
        case 'Digit3':
        case 'Numpad3':
            player.seek(Math.round(player.duration * 0.75));
            return true;
        case 'Tab':
        case 'KeyT':
            store.focusInputSignal.value++;
            return true;
    }

    const selected = store.selectedTrack();
    if (!selected) return false;

    // Done: hint an immediate flush and move on to the next untagged track
    if (event.code === 'KeyD' && !event.ctrlKey && !event.metaKey) {
        $1t.send('flushHint', { trackId: selected.id });
        store.selectNextUntagged();
        return true;
    }

    // Per-tag hotkeys from the pool
    for (const tag of store.tags.value) {
        if (tag.keybind && Keybind.check(event, Keybind.fromJson(tag.keybind))) {
            store.toggleTag(selected.id, tag);
            return true;
        }
    }

    return false;
}

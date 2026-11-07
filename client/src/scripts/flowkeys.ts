import { Notify } from 'quasar';
import { get1t } from './onetagger';
import { getTagStore } from './tagstore';
import { Keybind } from './utils';

/** Key codes the FlowTag view claims; per-tag hotkeys must avoid these */
export const RESERVED_FLOWTAG_CODES = [
    'Space', 'KeyJ', 'KeyK', 'KeyT', 'KeyD', 'KeyE', 'KeyM',
    'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight',
    'Digit0', 'Digit1', 'Digit2', 'Digit3',
    'Numpad0', 'Numpad1', 'Numpad2', 'Numpad3',
    'Escape', 'Tab',
];

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
 *             E edit file metadata · M online metadata lookup ·
 *             per-tag hotkeys toggle their tag on the selected track
 */
export function handleFlowKeys(event: KeyboardEvent): boolean {
    // Never act behind an open dialog (export confirm, settings, tag manager...)
    if (document.querySelector('.q-dialog')) return false;

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
            // Shift extends the multi-selection while moving
            store.selectOffset(-1, event.shiftKey);
            return true;
        case 'ArrowDown':
        case 'KeyK':
            store.selectOffset(1, event.shiftKey);
            return true;
        case 'Escape':
            if (store.selectedIds.value.length > 0) {
                store.clearMultiSelect();
                return true;
            }
            return false;
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
        // NOTE: Tab is deliberately NOT bound — it keeps normal focus navigation
        case 'KeyT':
            store.focusInputSignal.value++;
            return true;
        case 'KeyF':
            if (event.metaKey || event.ctrlKey) return false; // keep browser find-ish combos free
            store.focusFilterSignal.value++;
            return true;
    }

    // Undo the last tag edit
    if (event.code === 'KeyZ' && (event.metaKey || event.ctrlKey) && !event.shiftKey) {
        const undone = store.undo();
        if (undone) Notify.create({ message: `Undo: ${undone}`, timeout: 1800 });
        return true;
    }

    const selected = store.selectedTrack();
    if (!selected) return false;

    // Done: hint an immediate flush and move on to the next untagged track,
    // which starts playing (spec: done = save + advance + autoplay)
    if (event.code === 'KeyD' && !event.ctrlKey && !event.metaKey) {
        $1t.send('flushHint', { trackId: selected.id });
        player.wasPlaying = true;
        store.selectNextUntagged();
        return true;
    }

    // Track inspector (edit file metadata) / online lookup for the selected track
    if (event.code === 'KeyE' && !event.ctrlKey && !event.metaKey) {
        store.inspectorSignal.value++;
        return true;
    }
    if (event.code === 'KeyM' && !event.ctrlKey && !event.metaKey) {
        store.manualTagSignal.value++;
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

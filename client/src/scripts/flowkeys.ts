import { Notify } from 'quasar';
import { get1t } from './onetagger';
import { getTagStore } from './tagstore';
import { Keybind } from './utils';
import { diag } from './diag';
import { FlowTagAction } from './settings';

/** Default key (KeyboardEvent.code) per remappable action */
export const FLOWTAG_ACTION_DEFAULTS: Record<FlowTagAction, string> = {
    prev: 'KeyJ',
    next: 'KeyK',
    focusTag: 'KeyT',
    focusFilter: 'KeyF',
    done: 'KeyD',
    edit: 'KeyE',
    lookup: 'KeyM',
    sidebar: 'KeyB',
};

/** Labels for the settings page */
export const FLOWTAG_ACTION_LABELS: Record<FlowTagAction, string> = {
    prev: 'Previous track',
    next: 'Next track',
    focusTag: 'Focus tag input',
    focusFilter: 'Focus filter',
    done: 'Done & next untagged',
    edit: 'Edit metadata',
    lookup: 'Online lookup',
    sidebar: 'Toggle playlists sidebar',
};

/** Key codes FlowTag always claims, regardless of configuration */
export const FIXED_FLOWTAG_CODES = [
    'Space',
    'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight',
    'Digit0', 'Digit1', 'Digit2', 'Digit3',
    'Numpad0', 'Numpad1', 'Numpad2', 'Numpad3',
    'Escape', 'Tab',
];

/** Codes per-tag hotkeys must avoid: the fixed keys plus every current
 *  modifier-less action bind (a bind with a modifier can't clash with a
 *  plain tag hotkey, which TagManagerRow only checks when modifier-less). */
export function reservedFlowtagCodes(): string[] {
    const binds = get1t().settings.value.flowtag.keybinds;
    const codes = [...FIXED_FLOWTAG_CODES];
    for (const action of Object.keys(FLOWTAG_ACTION_DEFAULTS) as FlowTagAction[]) {
        const kb = binds[action];
        if (!kb || !kb.key) {
            codes.push(FLOWTAG_ACTION_DEFAULTS[action]);
        } else if (!kb.ctrl && !kb.alt && !kb.shift) {
            // Pseudo-code form; consumers normalize ('Keyn' -> 'n', 'Key1' -> '1')
            codes.push(`Key${kb.key}`);
        }
    }
    return codes;
}

/** The action's custom bind, revived, or undefined when it uses the default */
function customBind(action: FlowTagAction): Keybind | undefined {
    const kb = get1t().settings.value.flowtag.keybinds[action];
    return (kb && kb.key) ? Keybind.fromJson(kb) : undefined;
}

/** Does this event trigger the action? Default keys require no modifiers. */
function actionMatches(event: KeyboardEvent, action: FlowTagAction): boolean {
    const kb = customBind(action);
    if (kb) return !!Keybind.check(event, kb);
    return event.code === FLOWTAG_ACTION_DEFAULTS[action]
        && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
}

/** prev/next matching; shift on top of the bind extends the multi-selection */
function navMatches(event: KeyboardEvent, action: 'prev' | 'next'): { extend: boolean } | undefined {
    const kb = customBind(action);
    if (kb) {
        if (Keybind.check(event, kb)) return { extend: false };
        if (!kb.shift) {
            const shifted = Object.assign(Keybind.fromJson(kb), { shift: true });
            if (Keybind.check(event, shifted)) return { extend: true };
        }
        return undefined;
    }
    if (event.code !== FLOWTAG_ACTION_DEFAULTS[action]
        || event.ctrlKey || event.metaKey || event.altKey) return undefined;
    return { extend: event.shiftKey };
}

/**
 * Keyboard map for the FlowTag (Quick Tag) view.
 *
 * Called from the global keydown handler when the route is /quicktag and the
 * event target is not an input. Returns true when the key was handled (the
 * caller then prevents default).
 *
 * Fixed:        Space play/pause · ↑/↓ select (shift extends) ·
 *               ←/→ seek -10s/+30s · 1/2/3 seek 25/50/75% · 0 seek start ·
 *               Esc clear multi-select · ⌘Z undo
 * Remappable:   prev J · next K · focus tag input T · focus filter F ·
 *               done D · edit metadata E · online lookup M
 *               (Settings → Keyboard; per-tag hotkeys live in Manage tags)
 */
export function handleFlowKeys(event: KeyboardEvent): boolean {
    // Never act behind an open dialog (export confirm, settings, tag manager...)
    if (document.querySelector('.q-dialog')) return false;

    const $1t = get1t();
    const store = getTagStore();
    const player = $1t.player.value;

    // Fixed transport keys work with or without a selection
    switch (event.code) {
        case 'Space': {
            // Selection is pure; Space is the explicit "play this" — it starts
            // the selected row when that isn't the loaded track
            const sel = store.selectedTrack();
            if (sel && !sel.missing && player.path !== sel.path) {
                player.startTrack(sel.path, sel.id, sel.title, sel.artist, true);
            } else if (player.playing) {
                player.pause();
            } else {
                player.play();
            }
            return true;
        }
        case 'ArrowUp':
            // Shift extends the multi-selection while moving
            store.selectOffset(-1, event.shiftKey);
            return true;
        case 'ArrowDown':
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
    }

    // Undo the last tag edit (fixed)
    if (event.code === 'KeyZ' && (event.metaKey || event.ctrlKey) && !event.shiftKey) {
        const undone = store.undo();
        if (undone) Notify.create({ message: `Undo: ${undone}`, timeout: 1800 });
        return true;
    }

    // Remappable actions that work without a selection
    const prev = navMatches(event, 'prev');
    if (prev) {
        store.selectOffset(-1, prev.extend);
        return true;
    }
    const next = navMatches(event, 'next');
    if (next) {
        store.selectOffset(1, next.extend);
        return true;
    }
    if (actionMatches(event, 'focusTag')) {
        store.focusInputSignal.value++;
        return true;
    }
    if (actionMatches(event, 'focusFilter')) {
        store.focusFilterSignal.value++;
        return true;
    }
    if (actionMatches(event, 'sidebar')) {
        store.sidebarSignal.value++;
        return true;
    }

    const selected = store.selectedTrack();
    if (!selected) return false;

    // Done: hint an immediate flush and move on to the next untagged track,
    // which starts playing (spec: done = save + advance + autoplay)
    if (actionMatches(event, 'done')) {
        diag('done', { trackId: selected.id, tags: selected.tagIds.length });
        $1t.send('flushHint', { trackId: selected.id });
        player.wasPlaying = true;
        store.selectNextUntagged();
        return true;
    }

    // Track inspector (edit file metadata) / online lookup for the selected track
    if (actionMatches(event, 'edit')) {
        store.inspectorSignal.value++;
        return true;
    }
    if (actionMatches(event, 'lookup')) {
        store.manualTagSignal.value++;
        return true;
    }

    // Per-tag hotkeys from the pool (action binds take precedence above)
    for (const tag of store.tags.value) {
        if (tag.keybind && Keybind.check(event, Keybind.fromJson(tag.keybind))) {
            store.toggleTag(selected.id, tag);
            return true;
        }
    }

    return false;
}

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const settings = vi.hoisted(() => ({ keybinds: {} as Record<string, unknown> }));
const send = vi.hoisted(() => vi.fn());
const playerStub = vi.hoisted(() => ({} as Record<string, unknown>));
vi.mock('@/scripts/onetagger', () => ({
    get1t: () => ({ send, settings: { value: { flowtag: settings } }, player: { value: playerStub } }),
}));
vi.mock('@/lib/notify', () => ({ notify: () => undefined }));

import { getTagStore, FlowTag } from '@/scripts/tagstore';
import { actionFires, handleFlowKeys } from '@/scripts/flowkeys';
import { isCommitChord, tagInputOwnsKey } from '@/scripts/commitsheet';

function key(code: string, mods: Partial<KeyboardEvent> = {}): KeyboardEvent {
    return { code, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...mods } as KeyboardEvent;
}

function tag(keybind?: { key: string; ctrl?: boolean; alt?: boolean; shift?: boolean }): FlowTag {
    return { id: 1, name: 't', color: '#fff', count: 0, keybind } as unknown as FlowTag;
}

beforeEach(() => {
    settings.keybinds = {};
    getTagStore().tags.value = [];
});

describe('digit seek', () => {
    afterEach(() => vi.unstubAllGlobals());

    it('every_digit_seeks_to_its_tenth_of_the_track', () => {
        vi.stubGlobal('document', { querySelector: () => null });
        const seek = vi.fn();
        Object.assign(playerStub, { seek, duration: 200_000, position: 0 });
        expect(handleFlowKeys(key('Digit7') as KeyboardEvent)).toBe(true);
        expect(seek).toHaveBeenLastCalledWith(140_000);
        handleFlowKeys(key('Numpad4') as KeyboardEvent);
        expect(seek).toHaveBeenLastCalledWith(80_000);
        handleFlowKeys(key('Digit0') as KeyboardEvent);
        expect(seek).toHaveBeenLastCalledWith(0);
    });
});

/** Fake elements as attribute maps; a minimal matcher for the overlay selector's
 *  grammar: comma lists of one or more `[attr="v"]` conjuncts */
type FakeEl = Record<string, string>;
function matches(el: FakeEl, part: string): boolean {
    const conjuncts = part.trim().match(/^(\[[\w-]+="[^"]*"\])+$/);
    if (!conjuncts) throw new Error(`matcher cannot parse: ${part}`);
    return [...part.matchAll(/\[([\w-]+)="([^"]*)"\]/g)].every(([, attr, value]) => el[attr] === value);
}
function fakeDocument(els: FakeEl[]) {
    return { querySelector: (sel: string) => els.find((el) => sel.split(',').some((p) => matches(el, p))) ?? null };
}

describe('overlay guard', () => {
    afterEach(() => vi.unstubAllGlobals());
    const trackList: FakeEl = { role: 'listbox' };
    const open = (slot: string): FakeEl => ({ 'data-slot': slot, 'data-state': 'open' });

    function pressDigit(els: FakeEl[]) {
        Object.assign(playerStub, { seek: vi.fn(), duration: 100_000, position: 0 });
        vi.stubGlobal('document', fakeDocument(els));
        return handleFlowKeys(key('Digit5'));
    }

    it('the_always_mounted_track_list_listbox_does_not_block_shortcuts', () => {
        expect(pressDigit([trackList])).toBe(true);
    });

    it('any_unmarked_always_mounted_listbox_or_menu_does_not_block_shortcuts', () => {
        expect(pressDigit([{ role: 'listbox' }, { role: 'menu' }])).toBe(true);
    });

    it('an_open_select_blocks_shortcuts', () => {
        expect(pressDigit([trackList, open('select-content')])).toBe(false);
    });

    it('an_open_dialog_or_alert_dialog_blocks_shortcuts', () => {
        expect(pressDigit([trackList, open('dialog-content')])).toBe(false);
        expect(pressDigit([trackList, open('alert-dialog-content')])).toBe(false);
    });

    it('an_open_menu_or_popover_blocks_shortcuts', () => {
        expect(pressDigit([open('dropdown-menu-content')])).toBe(false);
        expect(pressDigit([open('popover-content')])).toBe(false);
        expect(pressDigit([open('context-menu-content')])).toBe(false);
    });

    it('an_overlay_in_its_closing_animation_no_longer_blocks', () => {
        expect(pressDigit([{ 'data-slot': 'select-content', 'data-state': 'closed' }])).toBe(true);
    });
});

describe('default stage/skip bindings yield', () => {
    it('stage_fires_when_s_is_unclaimed', () => {
        expect(actionFires(key('KeyS'), 'stage')).toBe(true);
        expect(actionFires(key('KeyX'), 'skip')).toBe(true);
        expect(actionFires(key('KeyS', { metaKey: true }), 'stage')).toBe(false);
    });

    it('stage_yields_to_a_tag_hotkey_on_s', () => {
        getTagStore().tags.value = [tag({ key: 's' })];
        expect(actionFires(key('KeyS'), 'stage')).toBe(false);
        expect(actionFires(key('KeyX'), 'skip')).toBe(true);
    });

    it('stage_yields_to_another_actions_custom_bind', () => {
        settings.keybinds = { edit: { key: 's', ctrl: false, alt: false, shift: false } };
        expect(actionFires(key('KeyS'), 'stage')).toBe(false);
    });

    it('custom_stage_bind_wins_normally', () => {
        settings.keybinds = { stage: { key: 'w', ctrl: false, alt: false, shift: false } };
        expect(actionFires(key('KeyW'), 'stage')).toBe(true);
        expect(actionFires(key('KeyS'), 'stage')).toBe(false);
    });
});

describe('commit chord in the tag input', () => {
    it('cmd_enter_is_left_to_the_global_handler', () => {
        expect(isCommitChord(key('Enter', { metaKey: true }))).toBe(true);
        expect(isCommitChord(key('Enter', { ctrlKey: true }))).toBe(true);
        // the input must neither swallow nor stop propagation of the chord
        expect(tagInputOwnsKey(key('Enter', { metaKey: true }))).toBe(false);
        expect(tagInputOwnsKey(key('Enter', { ctrlKey: true }))).toBe(false);
    });

    it('plain_and_shift_enter_stay_in_the_input', () => {
        expect(tagInputOwnsKey(key('Enter'))).toBe(true);
        expect(tagInputOwnsKey(key('Enter', { shiftKey: true }))).toBe(true);
        expect(tagInputOwnsKey(key('KeyS'))).toBe(true);
    });
});

describe('key repeat on stage / skip', () => {
    beforeEach(() => {
        send.mockClear();
        vi.stubGlobal('document', { querySelector: () => null });
        const store = getTagStore();
        store.tracks.value = [{
            id: 1, path: '/m/1.mp3', folderId: 1, format: 'mp3', title: 'T', artist: 'A',
            addedAt: 0, missing: false, needsFlush: false, tagIds: [],
        }];
        store.selectedIds.value = [];
        store.selectedId.value = 1;
    });
    afterEach(() => vi.unstubAllGlobals());

    it('held_s_and_x_are_consumed_without_toggling', () => {
        expect(handleFlowKeys(key('KeyS', { repeat: true }))).toBe(true);
        expect(handleFlowKeys(key('KeyX', { repeat: true }))).toBe(true);
        expect(send).not.toHaveBeenCalled();
    });

    it('a_fresh_press_still_toggles', () => {
        expect(handleFlowKeys(key('KeyS'))).toBe(true);
        expect(handleFlowKeys(key('KeyX'))).toBe(true);
        expect(send.mock.calls.map((c) => c[0])).toEqual(['stageTracks', 'stageSkipTracks']);
    });
});

describe('star rating keys', () => {
    const t = (id: number, rating: number | null = null) => ({
        id, path: `/m/${id}.mp3`, folderId: 1, format: 'mp3', title: 'T', artist: 'A',
        addedAt: 0, missing: false, needsFlush: false, tagIds: [], rating,
    });
    let seek: ReturnType<typeof vi.fn>;
    beforeEach(() => {
        send.mockClear();
        seek = vi.fn();
        Object.assign(playerStub, { seek, duration: 100_000, position: 0 });
        vi.stubGlobal('document', { querySelector: () => null });
        vi.stubGlobal('window', { location: { hash: '' } }); // diag breadcrumbs
        const store = getTagStore();
        store.tracks.value = [t(1), t(2, 3)];
        store.selectedIds.value = [];
        store.selectedId.value = 1;
    });
    afterEach(() => vi.unstubAllGlobals());
    /** Server sends other than diag breadcrumbs (clientLog) */
    const sent = () => send.mock.calls.filter((c) => c[0] !== 'clientLog');
    const lastRating = () => sent().filter((c) => c[0] === 'tracksSetRating').at(-1)?.[1];

    it('shift_digit_rates_the_selection_and_never_seeks', () => {
        expect(handleFlowKeys(key('Digit4', { shiftKey: true }))).toBe(true);
        expect(lastRating()).toEqual({ trackIds: [1], rating: 4 });
        expect(getTagStore().tracks.value[0]).toMatchObject({ rating: 4, needsFlush: true });
        expect(seek).not.toHaveBeenCalled();
    });

    it('pressing_the_same_rating_again_clears', () => {
        handleFlowKeys(key('Digit4', { shiftKey: true }));
        handleFlowKeys(key('Digit4', { shiftKey: true }));
        expect(lastRating()).toEqual({ trackIds: [1], rating: null });
        expect(getTagStore().tracks.value[0].rating).toBeNull();
    });

    it('multi_selection_clears_only_when_all_share_the_rating', () => {
        const store = getTagStore();
        store.selectedIds.value = [1, 2];
        handleFlowKeys(key('Digit3', { shiftKey: true }));
        expect(lastRating()).toEqual({ trackIds: [1, 2], rating: 3 });
        handleFlowKeys(key('Digit3', { shiftKey: true }));
        expect(lastRating()).toEqual({ trackIds: [1, 2], rating: null });
    });

    it('shift_six_to_zero_neither_rate_nor_seek', () => {
        for (const code of ['Digit6', 'Digit9', 'Digit0', 'Numpad5']) {
            expect(handleFlowKeys(key(code, { shiftKey: true }))).toBe(false);
        }
        expect(seek).not.toHaveBeenCalled();
        expect(sent()).toEqual([]);
    });

    it('plain_digits_still_seek', () => {
        expect(handleFlowKeys(key('Digit4'))).toBe(true);
        expect(seek).toHaveBeenLastCalledWith(40_000);
        expect(sent()).toEqual([]);
    });

    it('held_chord_does_not_toggle_at_repeat_rate', () => {
        expect(handleFlowKeys(key('Digit2', { shiftKey: true, repeat: true }))).toBe(true);
        expect(sent()).toEqual([]);
    });

    it('an_open_overlay_blocks_rating', () => {
        vi.stubGlobal('document', fakeDocument([{ 'data-slot': 'dialog-content', 'data-state': 'open' }]));
        expect(handleFlowKeys(key('Digit2', { shiftKey: true }))).toBe(false);
        expect(sent()).toEqual([]);
    });

    it('no_selection_is_unhandled_and_does_not_seek', () => {
        getTagStore().selectedId.value = undefined;
        expect(handleFlowKeys(key('Digit2', { shiftKey: true }))).toBe(false);
        expect(seek).not.toHaveBeenCalled();
    });

    it('an_existing_tag_hotkey_on_the_chord_keeps_it', () => {
        const store = getTagStore();
        store.tags.value = [{ ...tag({ key: '2', shift: true }), id: 7, name: 'warm' }];
        expect(handleFlowKeys(key('Digit2', { shiftKey: true }))).toBe(true);
        expect(sent().map((c) => c[0])).toEqual(['trackAddTag']);
    });
});

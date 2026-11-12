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
        expect(send.mock.calls.map((c) => c[0])).toEqual(['stageTracks', 'stageSkip']);
    });
});

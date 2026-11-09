<template>
<div class='tag-row flex items-center gap-2 px-2 py-1'>
    <ColorPicker :model-value='tag.color' @update:model-value='setColor' />

    <!-- Name (Enter commits, Esc reverts) -->
    <input
        v-model='draft'
        class='ft-inline-input flex-1'
        spellcheck='false'
        title='Press Enter to rename'
        :aria-label='`Rename ${tag.name}`'
        @keyup.enter='commitName'
        @keyup.esc.stop='draft = tag.name'
        @blur='draft = tag.name'
    />

    <!-- Usage -->
    <span class='count ft-num' :title='`${tag.count} tracks`'>{{ tag.count }}</span>

    <!-- Group -->
    <OptionSelect
        :model-value='tag.groupId ?? NO_GROUP'
        :options='groupOptions'
        label='Group'
        size='sm'
        trigger-class='w-[130px]'
        @update:model-value='setGroup'
    />

    <!-- Hotkey -->
    <div class='hotkey'>
        <KeybindVue :key='keybindNonce' :model-value='tag.keybind' @update:model-value='setKeybind' />
    </div>

    <!-- Delete -->
    <Hint text='Delete tag'>
        <Button size='icon-sm' variant='destructive' :aria-label='`Delete ${tag.name}`' @click='confirmDelete'>
            <TrashIcon class='size-3.5' />
        </Button>
    </Hint>
</div>
</template>

<script lang='ts' setup>
import { computed, ref, watch } from 'vue';
import { notify } from '@/lib/notify';
import { TrashIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { confirmDialog } from '@/lib/dialogs';
import ColorPicker from './ColorPicker.vue';
import Hint from './Hint.vue';
import OptionSelect from './OptionSelect.vue';
import { get1t } from '../../scripts/onetagger';
import { FlowTag, FlowGroup, getTagStore } from '../../scripts/tagstore';
import { reservedFlowtagCodes } from '../../scripts/flowkeys';
import KeybindVue from '../Keybind.vue';

const props = defineProps<{ tag: FlowTag, groups: FlowGroup[] }>();

const $1t = get1t();
const store = getTagStore();
const draft = ref(props.tag.name);

// Follow server-side changes (rename echo, merge) unless the user is typing
watch(() => props.tag.name, (name) => { draft.value = name; });

/** Select values can't be null: a sentinel stands for "no group" */
const NO_GROUP = -1;
const groupOptions = computed(() => [
    { label: 'None', value: NO_GROUP },
    ...props.groups.map((g) => ({ label: g.name, value: g.id })),
]);

function setColor(color: string) {
    if (color && color !== props.tag.color) {
        $1t.send('tagUpdate', { id: props.tag.id, color });
    }
}

function setGroup(groupId: number) {
    if (groupId === NO_GROUP) $1t.send('tagUpdate', { id: props.tag.id, clearGroup: true });
    else $1t.send('tagUpdate', { id: props.tag.id, groupId });
}

/** Keybind.key is stored like KeyboardEvent.code lowercased without the Key/Digit/Numpad prefix ('j', '1', 'f5') */
function normalizeCode(code: string): string {
    return code.toLowerCase().replace('key', '').replace('digit', '').replace('numpad', '');
}

// Bumped to remount the picker so a rejected recording isn't shown as set
const keybindNonce = ref(0);

function reject(message: string) {
    notify({ type: 'warning', message });
    keybindNonce.value++;
}

function setKeybind(keybind?: any) {
    if (!keybind) {
        $1t.send('tagUpdate', { id: props.tag.id, clearKeybind: true });
        return;
    }
    const hasModifier = !!(keybind.ctrl || keybind.alt || keybind.shift);
    // Recomputed per attempt: shortcut binds are configurable in Settings
    const reserved = new Set(reservedFlowtagCodes().map(normalizeCode));
    if (!hasModifier && reserved.has(String(keybind.key))) {
        reject('That key is used by a FlowTag shortcut or transport key. Add a modifier or pick another.');
        return;
    }
    const clash = store.tags.value.find((t) => t.id !== props.tag.id && t.keybind
        && t.keybind.key === keybind.key
        && !!t.keybind.ctrl === !!keybind.ctrl
        && !!t.keybind.alt === !!keybind.alt
        && !!t.keybind.shift === !!keybind.shift);
    if (clash) {
        reject(`That hotkey is already used by "${clash.name}".`);
        return;
    }
    $1t.send('tagUpdate', { id: props.tag.id, keybind: JSON.parse(JSON.stringify(keybind)) });
}

/**
 * Rename. When the new name equals another tag (case-insensitive) the server
 * merges this tag into it, so ask first; afterwards reload pool + tracks since
 * the merged-away tag id is gone from the DB.
 */
async function commitName() {
    const name = draft.value.trim();
    if (!name || name === props.tag.name) {
        draft.value = props.tag.name;
        return;
    }
    const other = store.tagByName(name);
    if (other && other.id !== props.tag.id) {
        const ok = await confirmDialog({
            title: 'Merge tags',
            message: `Merge "${props.tag.name}" into "${other.name}"? ${props.tag.count} track(s) affected.`,
            okLabel: 'Merge',
        });
        if (!ok) {
            draft.value = props.tag.name;
            return;
        }
        $1t.send('tagUpdate', { id: props.tag.id, name });
        $1t.send('tagPoolLoad');
        $1t.send('tagTracksLoad');
        return;
    }
    $1t.send('tagUpdate', { id: props.tag.id, name });
}

async function confirmDelete() {
    const ok = await confirmDialog({
        title: 'Delete tag',
        message: `Delete "${props.tag.name}"? It will be removed from ${props.tag.count} track(s).`,
        okLabel: 'Delete',
        destructive: true,
    });
    if (ok) $1t.send('tagDelete', { id: props.tag.id });
}
</script>

<style lang='scss'>
.tag-row {
    border-bottom: 1px solid var(--ft-line-faint);

    .count {
        min-width: 28px;
        padding: 1px 6px;
        border-radius: 4px;
        background: var(--ft-raised);
        color: var(--ft-text-2);
        font-size: 11px;
        text-align: center;
    }
    .hotkey { width: 110px; min-height: 32px; display: flex; align-items: center; }
}
</style>

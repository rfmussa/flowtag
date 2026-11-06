<template>
<div class='tag-row row items-center no-wrap q-px-sm q-py-xs'>

    <!-- Color swatch -->
    <q-btn
        round unelevated size='sm'
        class='swatch q-mr-sm'
        :style='{ backgroundColor: tag.color }'
        @click='blur'
    >
        <q-popup-proxy class='bg-darker'>
            <q-color
                :model-value='tag.color'
                no-header-tabs
                @change='setColor'
            />
        </q-popup-proxy>
        <q-tooltip>Change color</q-tooltip>
    </q-btn>

    <!-- Name (Enter commits) -->
    <q-input
        v-model='draft'
        dense borderless
        class='name-input col'
        input-class='name-field'
        @keyup.enter='commitName'
        @keyup.esc.stop='draft = tag.name'
        @blur='draft = tag.name'
    >
        <q-tooltip :delay='800'>Press Enter to rename</q-tooltip>
    </q-input>

    <!-- Usage -->
    <q-badge color='grey-8' class='q-mx-sm count' :label='tag.count' :title='`${tag.count} tracks`' />

    <!-- Group -->
    <q-select
        :model-value='tag.groupId ?? null'
        :options='groupOptions'
        emit-value map-options
        dense outlined options-dense
        class='group-select q-mr-sm'
        popup-content-class='bg-darker'
        @update:model-value='setGroup'
    />

    <!-- Hotkey -->
    <div class='hotkey q-mr-xs'>
        <KeybindVue :model-value='tag.keybind' @update:model-value='setKeybind' />
    </div>

    <!-- Delete -->
    <q-btn flat round dense size='sm' icon='mdi-delete' color='red' @click='confirmDelete'>
        <q-tooltip>Delete tag</q-tooltip>
    </q-btn>
</div>
</template>

<script lang='ts' setup>
import { computed, ref, watch } from 'vue';
import { Dialog } from 'quasar';
import { get1t } from '../../scripts/onetagger';
import { FlowTag, FlowGroup, getTagStore } from '../../scripts/tagstore';
import KeybindVue from '../Keybind.vue';

const props = defineProps<{ tag: FlowTag, groups: FlowGroup[] }>();

const $1t = get1t();
const store = getTagStore();
const draft = ref(props.tag.name);

// Follow server-side changes (rename echo, merge) unless the user is typing
watch(() => props.tag.name, (name) => { draft.value = name; });

const groupOptions = computed(() => [
    { label: 'None', value: null },
    ...props.groups.map((g) => ({ label: g.name, value: g.id })),
]);

/** Release focus so tagging hotkeys keep working */
function blur(e: Event) {
    (e.currentTarget as HTMLElement | null)?.blur();
}

function setColor(color: string) {
    if (color && color !== props.tag.color) {
        $1t.send('tagUpdate', { id: props.tag.id, color });
    }
}

function setGroup(groupId: number | null) {
    if (groupId === null) $1t.send('tagUpdate', { id: props.tag.id, clearGroup: true });
    else $1t.send('tagUpdate', { id: props.tag.id, groupId });
}

function setKeybind(keybind?: object) {
    if (keybind) $1t.send('tagUpdate', { id: props.tag.id, keybind: JSON.parse(JSON.stringify(keybind)) });
    else $1t.send('tagUpdate', { id: props.tag.id, clearKeybind: true });
}

/**
 * Rename. When the new name equals another tag (case-insensitive) the server
 * merges this tag into it, so ask first; afterwards reload pool + tracks since
 * the merged-away tag id is gone from the DB.
 */
function commitName() {
    const name = draft.value.trim();
    if (!name || name === props.tag.name) {
        draft.value = props.tag.name;
        return;
    }
    const other = store.tagByName(name);
    if (other && other.id !== props.tag.id) {
        Dialog.create({
            title: 'Merge tags',
            message: `Merge "${props.tag.name}" into "${other.name}"? ${props.tag.count} track(s) affected.`,
            cancel: true,
            ok: { label: 'Merge', color: 'primary', flat: true },
            persistent: true,
        }).onOk(() => {
            $1t.send('tagUpdate', { id: props.tag.id, name });
            $1t.send('tagPoolLoad');
            $1t.send('tagTracksLoad');
        }).onCancel(() => { draft.value = props.tag.name; });
        return;
    }
    $1t.send('tagUpdate', { id: props.tag.id, name });
}

function confirmDelete() {
    Dialog.create({
        title: 'Delete tag',
        message: `Delete "${props.tag.name}"? It will be removed from ${props.tag.count} track(s).`,
        cancel: true,
        ok: { label: 'Delete', color: 'red', flat: true },
        persistent: true,
    }).onOk(() => $1t.send('tagDelete', { id: props.tag.id }));
}
</script>

<style lang='scss'>
.tag-row {
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);

    .swatch {
        width: 22px;
        height: 22px;
        min-height: 22px;
        border: 1px solid rgba(255, 255, 255, 0.25);
    }
    .name-input { min-width: 80px; }
    .count { min-width: 24px; justify-content: center; }
    .group-select { width: 130px; }
    .hotkey { width: 110px; min-height: 32px; }
}
</style>

<template>
<div class='group-manager'>
    <div class='ft-label q-px-sm q-mb-xs'>Groups</div>

    <div v-if='ordered.length === 0' class='text-grey-6 text-caption q-px-sm q-py-xs'>No groups yet</div>

    <draggable
        v-model='ordered'
        item-key='id'
        handle='.drag-handle'
        @end='persistOrder'
    >
        <template #item='{ element: group }'>
            <div class='group-row row items-center no-wrap q-px-sm q-py-xs'>
                <q-icon name='mdi-drag' class='drag-handle q-mr-xs cursor-pointer' size='sm' />

                <q-btn
                    round unelevated size='sm'
                    class='swatch q-mr-sm'
                    :style='{ backgroundColor: group.color }'
                    @click='blur'
                >
                    <q-popup-proxy class='flowtag-theme'>
                        <q-color
                            :model-value='group.color'
                            no-header-tabs
                            @change='(c: string) => setColor(group, c)'
                        />
                    </q-popup-proxy>
                </q-btn>

                <q-input
                    :model-value='drafts[group.id] ?? group.name'
                    dense borderless
                    class='col'
                    @update:model-value='(v: string | number | null) => drafts = { ...drafts, [group.id]: String(v ?? "") }'
                    @keyup.enter='rename(group)'
                    @blur='clearDraft(group.id)'
                >
                    <q-tooltip :delay='800'>Press Enter to rename</q-tooltip>
                </q-input>

                <q-btn flat round dense size='sm' icon='mdi-delete' color='red' @click='confirmDelete(group)'>
                    <q-tooltip>Delete group</q-tooltip>
                </q-btn>
            </div>
        </template>
    </draggable>

    <!-- Add group -->
    <div class='row items-center no-wrap q-px-sm q-pt-sm'>
        <q-btn
            round unelevated size='sm'
            class='swatch q-mr-sm'
            :style='{ backgroundColor: newColor }'
            @click='blur'
        >
            <q-popup-proxy class='flowtag-theme'>
                <q-color v-model='newColor' no-header-tabs />
            </q-popup-proxy>
        </q-btn>
        <q-input
            v-model='newName'
            dense outlined
            placeholder='New group name'
            class='col q-mr-sm'
            @keyup.enter='add'
        />
        <q-btn flat dense no-caps icon='mdi-plus' label='Add group' color='primary' :disable='!newName.trim()' @click='add' />
    </div>
</div>
</template>

<script lang='ts' setup>
import { computed, ref } from 'vue';
import { Dialog } from 'quasar';
import draggable from 'vuedraggable';
import { get1t } from '../../scripts/onetagger';
import { FlowGroup, getTagStore } from '../../scripts/tagstore';

const DEFAULT_GROUP_COLOR = '#6c8ebf';

const $1t = get1t();
const store = getTagStore();

const newName = ref('');
const newColor = ref(DEFAULT_GROUP_COLOR);
const drafts = ref<Record<number, string>>({});

// Sorted by position; assigning (from draggable) keeps a local order until the pool echo arrives
const localOrder = ref<FlowGroup[] | undefined>();
const ordered = computed<FlowGroup[]>({
    get: () => localOrder.value
        ?? [...store.groups.value].sort((a, b) => a.position - b.position || a.id - b.id),
    set: (v) => { localOrder.value = v; },
});

function blur(e: Event) {
    (e.currentTarget as HTMLElement | null)?.blur();
}

/** Send the new index as position for every group that moved */
function persistOrder() {
    const list = localOrder.value;
    if (!list) return;
    list.forEach((g, i) => {
        if (g.position !== i) $1t.send('groupUpdate', { id: g.id, position: i });
    });
    // Fresh pool arrives after each update; drop the local override once it has
    setTimeout(() => { localOrder.value = undefined; }, 300);
}

function setColor(group: FlowGroup, color: string) {
    if (color && color !== group.color) $1t.send('groupUpdate', { id: group.id, color });
}

function clearDraft(id: number) {
    const { [id]: _removed, ...rest } = drafts.value;
    drafts.value = rest;
}

function rename(group: FlowGroup) {
    const name = (drafts.value[group.id] ?? group.name).trim();
    clearDraft(group.id);
    if (name && name !== group.name) $1t.send('groupUpdate', { id: group.id, name });
}

function confirmDelete(group: FlowGroup) {
    const n = store.tags.value.filter((t) => t.groupId === group.id).length;
    Dialog.create({
        title: 'Delete group',
        message: `Delete "${group.name}"? Its ${n} tag(s) will become ungrouped.`,
        cancel: true,
        class: 'flowtag-theme',
        ok: { label: 'Delete', color: 'red', flat: true },
        persistent: true,
    }).onOk(() => $1t.send('groupDelete', { id: group.id }));
}

function add() {
    const name = newName.value.trim();
    if (!name) return;
    $1t.send('groupCreate', { name, color: newColor.value });
    newName.value = '';
}
</script>

<style lang='scss'>
.group-manager {
    .group-row { border-bottom: 1px solid rgba(255, 255, 255, 0.05); }
    .swatch {
        width: 22px;
        height: 22px;
        min-height: 22px;
        border: 1px solid rgba(255, 255, 255, 0.25);
    }
    .drag-handle { cursor: grab; opacity: 0.6; }
}
</style>

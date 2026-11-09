<template>
<div class='group-manager'>
    <div class='ft-label px-2 mb-1'>Groups</div>

    <div v-if='ordered.length === 0' class='px-2 py-1 text-xs text-ft-text-3'>No groups yet</div>

    <draggable
        v-model='ordered'
        item-key='id'
        handle='.drag-handle'
        @end='persistOrder'
    >
        <template #item='{ element: group }'>
            <div class='group-row flex items-center gap-2 px-2 py-1'>
                <GripVerticalIcon class='drag-handle size-4 shrink-0 text-ft-text-3' aria-label='Drag to reorder' />
                <ColorPicker :model-value='group.color' @update:model-value='(c: string) => setColor(group, c)' />
                <input
                    :value='drafts[group.id] ?? group.name'
                    class='ft-inline-input flex-1'
                    spellcheck='false'
                    title='Press Enter to rename'
                    :aria-label='`Rename ${group.name}`'
                    @input='drafts = { ...drafts, [group.id]: ($event.target as HTMLInputElement).value }'
                    @keyup.enter='rename(group)'
                    @blur='clearDraft(group.id)'
                />
                <Hint text='Delete group'>
                    <Button size='icon-sm' variant='destructive' :aria-label='`Delete ${group.name}`' @click='confirmDelete(group)'>
                        <TrashIcon class='size-3.5' />
                    </Button>
                </Hint>
            </div>
        </template>
    </draggable>

    <!-- Add group -->
    <form class='flex items-center gap-2 px-2 pt-2' @submit.prevent='add'>
        <span class='w-4 shrink-0'></span>
        <ColorPicker v-model='newColor' label='New group color' />
        <Input v-model='newName' placeholder='New group name' class='flex-1' spellcheck='false' />
        <Button type='submit' size='sm' class='text-ft-text' :disabled='!newName.trim()'><PlusIcon class='size-3.5' />Add group</Button>
    </form>
</div>
</template>

<script lang='ts' setup>
import { computed, ref } from 'vue';
import { GripVerticalIcon, PlusIcon, TrashIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { confirmDialog } from '@/lib/dialogs';
import ColorPicker from './ColorPicker.vue';
import Hint from './Hint.vue';
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

async function confirmDelete(group: FlowGroup) {
    const n = store.tags.value.filter((t) => t.groupId === group.id).length;
    const ok = await confirmDialog({
        title: 'Delete group',
        message: `Delete "${group.name}"? Its ${n} tag(s) will become ungrouped.`,
        okLabel: 'Delete',
        destructive: true,
    });
    if (ok) $1t.send('groupDelete', { id: group.id });
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
    .group-row { border-bottom: 1px solid var(--ft-line-faint); }
    .drag-handle { cursor: grab; opacity: 0.6; }
}
</style>

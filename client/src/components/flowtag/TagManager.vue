<template>
<Dialog v-model:open='open'>
    <DialogContent class='tag-manager flex flex-col gap-0 p-0 sm:max-w-[760px]' :show-close-button='false'>
        <div class='flex items-center gap-2 px-4 pt-4 pb-2'>
            <DialogTitle>Manage tags</DialogTitle>
            <DialogDescription class='sr-only'>Rename, recolor, group and assign hotkeys to tags</DialogDescription>
            <div class='flex-1'></div>
            <label class='ft-field w-[200px]'>
                <SearchIcon class='size-3.5 shrink-0 text-ft-text-3' />
                <input v-model='search' type='text' placeholder='Search tags' spellcheck='false' aria-label='Search tags' />
                <button v-if='search' type='button' class='grid place-items-center text-ft-text-3 hover:text-ft-text cursor-pointer' aria-label='Clear search' @click='search = ""'>
                    <XIcon class='size-3.5' />
                </button>
            </label>
            <DialogClose as-child>
                <Button size='icon' aria-label='Close'><XIcon /></Button>
            </DialogClose>
        </div>

        <!-- Tags -->
        <div class='flex-1 min-h-0 overflow-y-auto px-2 pb-2'>
            <div v-if='shown.length === 0' class='text-ft-text-3 text-sm text-center p-4'>
                {{ store.tags.value.length === 0 ? 'No tags yet. Tag a track to create one.' : 'No tags match' }}
            </div>
            <TagManagerRow
                v-for='tag in shown'
                :key='tag.id'
                :tag='tag'
                :groups='store.groups.value'
            />
        </div>

        <!-- Groups -->
        <div class='group-section shrink-0 overflow-y-auto border-t border-ft-line p-2'>
            <GroupManager />
        </div>
    </DialogContent>
</Dialog>
</template>

<script lang='ts' setup>
import { computed, ref } from 'vue';
import { useEventListener } from '@vueuse/core';
import { SearchIcon, XIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog';
import { getTagStore } from '../../scripts/tagstore';
import TagManagerRow from './TagManagerRow.vue';
import GroupManager from './GroupManager.vue';

const open = defineModel<boolean>({ default: false });
const store = getTagStore();
const search = ref('');

const shown = computed(() => {
    const q = (search.value ?? '').trim().toLowerCase();
    return [...store.tags.value]
        .filter((t) => !q || t.name.toLowerCase().includes(q))
        .sort((a, b) => a.name.localeCompare(b.name));
});

/**
 * The FlowTag keyboard map listens on `document`; while this dialog (or the
 * hotkey recorder inside it) is open, swallow keydowns at `body` so Space/J/K
 * and per-tag hotkeys don't fire behind it. Escape must still reach the dialog.
 */
function swallowKeys(e: KeyboardEvent) {
    if (e.key !== 'Escape') e.stopPropagation();
}
// Attached while the dialog is open; detached on close and on unmount
useEventListener(() => (open.value ? document.body : null), 'keydown', swallowKeys);
</script>

<style lang='scss'>
.tag-manager {
    height: 80vh;
    max-height: 80vh;
    .group-section { max-height: 34%; }
}
</style>

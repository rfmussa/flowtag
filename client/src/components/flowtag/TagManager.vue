<template>
<q-dialog v-model='open' @show='listen' @hide='unlisten'>
    <q-card class='tag-manager column no-wrap flowtag-theme'>
        <q-card-section class='row items-center q-pb-none'>
            <div class='text-h6'>Manage tags</div>
            <q-space />
            <q-input
                v-model='search'
                dense outlined clearable
                placeholder='Search tags'
                class='search-input q-mr-sm'
            />
            <q-btn flat round dense icon='mdi-close' v-close-popup />
        </q-card-section>

        <!-- Tags -->
        <q-card-section class='col scroll tag-list q-pa-sm'>
            <div v-if='shown.length === 0' class='text-grey-6 text-center q-pa-md'>
                {{ store.tags.value.length === 0 ? 'No tags yet. Tag a track to create one.' : 'No tags match' }}
            </div>
            <TagManagerRow
                v-for='tag in shown'
                :key='tag.id'
                :tag='tag'
                :groups='store.groups.value'
            />
        </q-card-section>

        <q-separator />

        <!-- Groups -->
        <q-card-section class='group-section scroll q-pa-sm'>
            <GroupManager />
        </q-card-section>
    </q-card>
</q-dialog>
</template>

<script lang='ts' setup>
import { computed, onBeforeUnmount, ref } from 'vue';
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
 * and per-tag hotkeys don't fire behind it. Escape must still reach Quasar.
 */
function swallowKeys(e: KeyboardEvent) {
    if (e.key !== 'Escape') e.stopPropagation();
}
function listen() { document.body.addEventListener('keydown', swallowKeys); }
function unlisten() { document.body.removeEventListener('keydown', swallowKeys); }
onBeforeUnmount(unlisten);
</script>

<style lang='scss'>
.tag-manager {
    width: 760px;
    max-width: 95vw;
    height: 80vh;
    max-height: 80vh;

    .search-input { width: 200px; }
    .group-section { max-height: 34%; flex-shrink: 0; }
}
</style>

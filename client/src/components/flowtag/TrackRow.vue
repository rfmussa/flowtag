<template>
<div class='flowtag-row row items-center no-wrap q-px-sm' :class='{"flowtag-row-selected": selected}' @click='emit("click")'>
    <div class='col' style='min-width: 0'>
        <div class='text-weight-bold ellipsis'>{{ track.title }}</div>
        <div class='text-caption text-grey-6 ellipsis'>{{ track.artist }}</div>
    </div>

    <div class='flowtag-row-right row items-center no-wrap q-pl-sm'>
        <TagChips dense :tagIds='track.tagIds'></TagChips>
        <q-icon v-if='track.needsFlush' name='mdi-circle-medium' color='orange' size='sm' class='q-ml-xs'>
            <q-tooltip>Not yet written to file</q-tooltip>
        </q-icon>
        <q-icon v-if='track.missing' name='mdi-file-alert-outline' color='grey-6' size='xs' class='q-ml-xs'>
            <q-tooltip>File missing</q-tooltip>
        </q-icon>
    </div>
</div>
</template>

<script lang='ts' setup>
import TagChips from './TagChips.vue';
import { FlowTrack } from '../../scripts/tagstore';

defineProps<{
    track: FlowTrack;
    selected: boolean;
}>();
const emit = defineEmits<{ (e: 'click'): void }>();
</script>

<style lang='scss'>
.flowtag-row {
    height: 48px;
    max-height: 48px;
    overflow: hidden;
    cursor: pointer;
    border-left: 3px solid transparent;
    box-sizing: border-box;
}
.flowtag-row:hover {
    background: rgba(255, 255, 255, 0.04);
}
.flowtag-row-selected,
.flowtag-row-selected:hover {
    background: rgba(var(--q-primary-rgb, 25, 118, 210), 0.15);
    border-left-color: var(--q-primary);
}
.flowtag-row-right {
    flex: 0 1 auto;
    max-width: 60%;
    overflow: hidden;
    white-space: nowrap;
}
</style>

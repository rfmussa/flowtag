<template>
<div class='flowtag-row row items-center no-wrap q-px-sm' :class='{"flowtag-row-selected": selected}' @click='emit("click", $event)'>
    <div class='col' style='min-width: 0'>
        <div class='text-weight-bold ellipsis'>{{ track.title }}</div>
        <div class='text-caption text-grey-6 ellipsis'>{{ track.artist }}</div>
    </div>

    <div class='flowtag-row-right row items-center no-wrap q-pl-sm'>
        <TagChips dense :tagIds='track.tagIds'></TagChips>
        <span class='meta text-caption q-ml-sm'>{{ track.year ?? '' }}</span>
        <span class='meta text-caption q-ml-sm'>{{ track.bpm ?? '' }}</span>
        <span class='meta meta-key text-caption q-ml-sm' :style='{ color: keyColor(track.key) }'>{{ track.key ?? '' }}</span>
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
import { keyColor } from '../../scripts/musickey';

defineProps<{
    track: FlowTrack;
    selected: boolean;
}>();
const emit = defineEmits<{ (e: 'click', ev: MouseEvent): void }>();
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

    .meta {
        color: #8b949e;
        min-width: 32px;
        text-align: right;
        font-variant-numeric: tabular-nums;
    }
    .meta-key {
        min-width: 30px;
        font-weight: 600;
    }
}
</style>

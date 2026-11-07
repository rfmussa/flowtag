<template>
<div
    class='flowtag-row'
    :class='{"flowtag-row-selected": selected, "flowtag-row-anchor": anchor}'
    @click='emit("click", $event)'
>
    <span class='row-title' :title='track.title'>{{ track.title }}</span>
    <span class='row-artist' :title='track.artist'>{{ track.artist }}</span>
    <span class='row-spacer'></span>
    <TagChips dense :tagIds='track.tagIds' class='row-chips'></TagChips>
    <span v-if='track.needsFlush' class='row-pending' title='Not yet written to file'></span>
    <q-icon v-if='track.missing' name='mdi-file-alert-outline' size='13px' class='row-missing'>
        <q-tooltip>File missing</q-tooltip>
    </q-icon>
    <span class='row-meta ft-num'>{{ track.year ?? '' }}</span>
    <span class='row-meta row-bpm ft-num'>{{ track.bpm ?? '' }}</span>
    <span class='row-key ft-mono' :style='{ color: track.key ? keyColor(track.key) : undefined }'>{{ track.key ?? '' }}</span>
</div>
</template>

<script lang='ts' setup>
import TagChips from './TagChips.vue';
import { FlowTrack } from '../../scripts/tagstore';
import { keyColor } from '../../scripts/musickey';

defineProps<{
    track: FlowTrack;
    selected: boolean;
    anchor?: boolean;
}>();
const emit = defineEmits<{ (e: 'click', ev: MouseEvent): void }>();
</script>

<style lang='scss'>
.flowtag-row {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    max-height: 40px;
    box-sizing: border-box;
    padding: 0 16px 0 14px;
    overflow: hidden;
    cursor: default;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    // Fixed-size rows: fence layout/paint so scrolling never invalidates neighbors
    contain: layout paint style;

    &:hover { background: var(--ft-hover); }

    .row-title {
        font-size: 13px;
        font-weight: 500;
        color: var(--ft-text);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        max-width: 42%;
        flex-shrink: 0;
    }
    .row-artist {
        font-size: 12.5px;
        color: var(--ft-text-3);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        min-width: 0;
        flex-shrink: 1;
    }
    .row-spacer { flex: 1; }
    .row-chips {
        flex-shrink: 0;
        max-width: 280px;
        overflow: hidden;
    }
    .row-pending {
        width: 5px;
        height: 5px;
        border-radius: 50%;
        background: var(--ft-warn);
        flex-shrink: 0;
    }
    .row-missing { color: var(--ft-text-3); flex-shrink: 0; }
    .row-meta {
        width: 34px;
        text-align: right;
        font-size: 11.5px;
        color: var(--ft-text-3);
        flex-shrink: 0;
    }
    .row-bpm { color: var(--ft-text-2); }
    .row-key {
        width: 30px;
        text-align: right;
        flex-shrink: 0;
        font-weight: 600;
    }
}

// Range-selected rows: a quiet fill
.flowtag-row-selected {
    background: var(--ft-active);
    &:hover { background: var(--ft-active); }
    .row-title { color: #ffffff; }
}

// The anchor (active) row: raised surface, hairline ring, no colored rail
.flowtag-row-anchor,
.flowtag-row-anchor:hover {
    background: var(--ft-raised);
    box-shadow: inset 0 0 0 1px var(--ft-line-strong);
    border-radius: 6px;
    border-bottom-color: transparent;
}
</style>

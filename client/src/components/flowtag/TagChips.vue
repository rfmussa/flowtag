<template>
<span class='flowtag-chips' :class='{"flowtag-chips-dense": dense}'>
    <span
        v-for='tag in capped.shown'
        :key='tag.id'
        class='flowtag-chip'
        :title='tag.name'
    >
        <span class='chip-dot' :style='{ background: tag.color }'></span>
        <span class='chip-name'>{{ tag.name }}</span>
        <button
            v-if='removable && trackId !== undefined'
            type='button'
            class='chip-x'
            :aria-label='`Remove ${tag.name}`'
            @click.stop='store.removeTag(trackId!, tag.id)'
        >×</button>
    </span>
    <span v-if='capped.more > 0' class='chips-more' :title='overflowNames'>+{{ capped.more }}</span>
</span>
</template>

<script lang='ts'>
/** Pick readable text color for a hex background (relative luminance) */
export function chipTextColor(bg: string): 'black' | 'white' {
    const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec((bg ?? '').trim());
    if (!m) return 'white';
    let hex = m[1];
    if (hex.length === 3) hex = hex.split('').map((c) => c + c).join('');
    const lin = (i: number) => {
        const c = parseInt(hex.slice(i, i + 2), 16) / 255;
        return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
    };
    const lum = 0.2126 * lin(0) + 0.7152 * lin(2) + 0.0722 * lin(4);
    return lum > 0.179 ? 'black' : 'white';
}
</script>

<script lang='ts' setup>
import { computed } from 'vue';
import { getTagStore, FlowTag } from '../../scripts/tagstore';
import { capChips } from '@/lib/layout';

const props = defineProps<{
    tagIds: number[];
    trackId?: number;
    removable?: boolean;
    dense?: boolean;
    /** Show at most this many chips, the rest as "+n" (narrow windows) */
    max?: number;
}>();

const store = getTagStore();

// Resolve ids against the pool, skipping unknown ones
const resolved = computed<FlowTag[]>(() => {
    const out: FlowTag[] = [];
    for (const id of props.tagIds) {
        const tag = store.tagById(id);
        if (tag) out.push(tag);
    }
    return out;
});

const capped = computed(() => capChips(resolved.value, props.max));
const overflowNames = computed(() => resolved.value.slice(capped.value.shown.length).map((t) => t.name).join(', '));
</script>

<style lang='scss'>
.flowtag-chips {
    display: inline-flex;
    flex-wrap: nowrap;
    align-items: center;
    gap: 4px;
}
.chips-more {
    font-size: 10.5px;
    color: var(--ft-text-3);
    flex-shrink: 0;
}
.flowtag-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 22px;
    padding: 0 8px;
    border-radius: 11px;
    border: 1px solid var(--ft-line);
    background: var(--ft-panel);
    flex-shrink: 0;
    max-width: 150px;

    .chip-dot {
        width: 7px;
        height: 7px;
        border-radius: 50%;
        flex-shrink: 0;
    }
    .chip-name {
        font-size: 11.5px;
        font-weight: 500;
        color: var(--ft-text-2);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .chip-x {
        border: none;
        background: none;
        padding: 0 0 0 1px;
        margin-right: -3px;
        font-size: 13px;
        line-height: 1;
        color: var(--ft-text-3);
        cursor: pointer;
        border-radius: 50%;
        width: 14px;
        height: 14px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        &:hover { color: var(--ft-text); background: var(--ft-active); }
    }
    &:hover { border-color: var(--ft-line-strong); .chip-name { color: var(--ft-text); } }
}
.flowtag-chips-dense .flowtag-chip {
    height: 20px;
    padding: 0 7px;
    .chip-name { font-size: 11px; }
    .chip-dot { width: 6px; height: 6px; }
}
</style>

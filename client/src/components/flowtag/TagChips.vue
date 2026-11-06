<template>
<span class='flowtag-chips'>
    <q-chip
        v-for='tag in resolved'
        :key='tag.id'
        dense
        square
        class='flowtag-chip'
        :class='{"flowtag-chip-dense": dense}'
        :style='{ backgroundColor: tag.color, color: chipTextColor(tag.color) }'
        :label='tag.name'
        :title='tag.name'
        :removable='removable && trackId !== undefined'
        @remove='store.removeTag(trackId!, tag.id)'
    ></q-chip>
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

const props = defineProps<{
    tagIds: number[];
    trackId?: number;
    removable?: boolean;
    dense?: boolean;
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
</script>

<style lang='scss'>
.flowtag-chips {
    display: inline-flex;
    flex-wrap: nowrap;
    align-items: center;
}
.flowtag-chip {
    border-radius: 4px;
    margin: 0 2px;
    flex-shrink: 0;
}
.flowtag-chip-dense {
    font-size: 11px;
}
</style>

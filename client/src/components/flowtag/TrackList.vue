<template>
<q-virtual-scroll
    ref='scroller'
    :items='visible'
    :virtual-scroll-item-size='40'
    :virtual-scroll-slice-size='40'
    :virtual-scroll-slice-ratio-before='2'
    :virtual-scroll-slice-ratio-after='2'
    class='track-scroll'
    v-slot='{ item }'
>
    <TrackRow
        :key='item.id'
        :track='item'
        :selected='store.isSelected(item.id)'
        :anchor='item.id === store.selectedId.value'
        @click='(ev) => select(item, ev)'
    />
</q-virtual-scroll>
</template>

<script lang='ts' setup>
import { computed, ref, watch } from 'vue';
import { QVirtualScroll } from 'quasar';
import { getTagStore, FlowTrack } from '../../scripts/tagstore';
import TrackRow from './TrackRow.vue';

const store = getTagStore();
const scroller = ref<QVirtualScroll>();

// Recomputes only when tracks / filter / toggle change
const visible = computed(() => {
    // Touch reactive deps explicitly so the computed tracks them
    store.filter.value;
    store.untaggedOnly.value;
    return store.visibleTracks();
});

function select(track: FlowTrack, ev: MouseEvent) {
    store.clickSelect(track, {
        toggle: ev.metaKey || ev.ctrlKey,
        range: ev.shiftKey,
    });
}

const ROW_H = 40;

// Keep the keyboard-selected row in view with the minimal scroll: already
// visible does nothing, one step off the edge nudges by a row (native-list
// cruise while holding J/K), and only a far jump (D to next untagged,
// filter change) recenters.
watch(store.selectedId, (id) => {
    if (id === undefined) return;
    const idx = visible.value.findIndex((t) => t.id === id);
    if (idx === -1) return;
    const el = scroller.value?.$el as HTMLElement | undefined;
    if (!el) {
        scroller.value?.scrollTo(idx, 'center');
        return;
    }
    const top = idx * ROW_H;
    const bottom = top + ROW_H;
    const viewTop = el.scrollTop;
    const viewBottom = viewTop + el.clientHeight;
    if (top >= viewTop && bottom <= viewBottom) return;
    const far = bottom < viewTop - el.clientHeight || top > viewBottom + el.clientHeight;
    scroller.value?.scrollTo(idx, far ? 'center' : (top < viewTop ? 'start' : 'end'));
});
</script>

<style lang='scss' scoped>
.track-scroll {
    height: 100%;
}
</style>

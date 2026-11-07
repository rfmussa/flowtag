<template>
<q-virtual-scroll
    ref='scroller'
    :items='visible'
    :virtual-scroll-item-size='40'
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

// Keep the keyboard-selected row in view
watch(store.selectedId, (id) => {
    if (id === undefined) return;
    const idx = visible.value.findIndex((t) => t.id === id);
    if (idx !== -1) scroller.value?.scrollTo(idx, 'center');
});
</script>

<style lang='scss' scoped>
.track-scroll {
    height: 100%;
}
</style>

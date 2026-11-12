<template>
<div ref='viewport' class='track-scroll'>
    <div class='track-spacer' :style='{ height: virtualizer.getTotalSize() + "px" }'>
        <TrackRow
            v-for='row in rows'
            v-show='!row.hidden'
            :key='row.slot'
            class='track-pooled'
            :style='{ transform: "translateY(" + row.start + "px)" }'
            :track='row.track'
            :selected='store.isSelected(row.track.id)'
            :anchor='row.track.id === store.selectedId.value'
            :alt='row.index % 2 === 1'
            :playing='row.track.path === playerPath'
            @click='(ev) => select(row.track, ev)'
            @play='emit("play", row.track)'
        />
    </div>
</div>
</template>

<script lang='ts' setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useVirtualizer } from '@tanstack/vue-virtual';
import { get1t } from '../../scripts/onetagger';
import { getTagStore, FlowTrack } from '../../scripts/tagstore';
import TrackRow from './TrackRow.vue';

/** Fixed row height (TanStack takes it as the exact size, no measuring) */
const ROW_H = 40;
/** Rows rendered beyond each edge of the viewport */
const OVERSCAN = 8;

const $1t = get1t();
const store = getTagStore();
const emit = defineEmits<{ (e: 'play', track: FlowTrack): void }>();

const viewport = ref<HTMLElement>();

// The actively playing file (may differ from the anchor with continue-playback)
const playerPath = computed(() => $1t.player.value.path);

// Recomputes only when tracks / filter / toggle change
const visible = computed(() => {
    // Touch reactive deps explicitly so the computed tracks them
    store.filter.value;
    store.quickFilter.value;
    store.scope.value;
    return store.visibleTracks();
});

// TanStack Virtual owns the scroll math: range, overscan, resize and
// scroll-element observation, scrollToIndex
// (used as-is: the adapter signals updates by re-triggering this same ref,
// which a computed wrapper would swallow)
const virtualizer = useVirtualizer(computed(() => ({
    count: visible.value.length,
    getScrollElement: () => viewport.value ?? null,
    estimateSize: () => ROW_H,
    overscan: OVERSCAN,
})));

/** Pool size: enough slots for the tallest possible viewport plus overscan.
 *  Fixed for the session, so slot assignment never reshuffles. */
const POOL = Math.ceil((window.screen?.availHeight || 1200) / ROW_H) + OVERSCAN * 2 + 2;

interface SlotRow { slot: number; index: number; start: number; track: FlowTrack; live: boolean; hidden: boolean }

/**
 * TanStack decides which rows are in the window and where they sit; this
 * layer only maps them onto a fixed pool of TrackRow components. Row i lives
 * in slot i % POOL, slots keep their order, and slots outside the window keep
 * their last track where they are (off screen) rather than being destroyed. Scrolling
 * therefore never creates or destroys a row component: keying by track would
 * mount a fresh row (chips, icons) for every row entering the viewport,
 * which measured ~39ms per step on a hard flick versus ~5ms pooled.
 */
const rows = computed<SlotRow[]>((previous) => {
    const list = visible.value;
    const bySlot: (SlotRow | undefined)[] = new Array(POOL);
    // Carry every slot that has ever rendered. A slot outside TanStack's
    // window always sits outside the viewport (its row is beyond overscan),
    // so it stays visible where it is: toggling display on every slot that
    // leaves the window forced style + layout of ~28 rows per flick step.
    // Only rows past the end of a shrunk list are hidden, so they can't
    // extend the scroll height or show below the content.
    for (const row of previous ?? []) {
        bySlot[row.slot] = { ...row, live: false, hidden: row.index >= list.length };
    }
    for (const item of virtualizer.value.getVirtualItems()) {
        const track = list[item.index];
        if (!track) continue;
        const slot = item.index % POOL;
        bySlot[slot] = { slot, index: item.index, start: item.start, track, live: true, hidden: false };
    }
    return bySlot.filter((r): r is SlotRow => r !== undefined);
});

function select(track: FlowTrack, ev: MouseEvent) {
    store.clickSelect(track, {
        toggle: ev.metaKey || ev.ctrlKey,
        range: ev.shiftKey,
    });
}

// Switching scope (playlist / source / collection) starts at the top
watch([store.scope, store.activeSourceId], () => {
    nextTick(() => virtualizer.value.scrollToOffset(0));
});

// Keep the keyboard-selected row in view: 'auto' scrolls only when the row
// is off screen and by the minimum (one row per J/K press while cruising);
// a far jump (D to next untagged) recenters instead.
watch(store.selectedId, (id) => {
    if (id === undefined) return;
    const idx = visible.value.findIndex((t) => t.id === id);
    if (idx === -1) return;
    const el = viewport.value;
    if (!el) return;
    const top = idx * ROW_H;
    const far = top + ROW_H < el.scrollTop - el.clientHeight
        || top > el.scrollTop + 2 * el.clientHeight;
    virtualizer.value.scrollToIndex(idx, { align: far ? 'center' : 'auto' });
});
</script>

<style lang='scss'>
// Unscoped: this class lands on TrackRow's root element
.track-scroll {
    height: 100%;
    overflow-y: auto;
    position: relative;
    // Spacer height changes must not trigger scroll anchoring jumps
    overflow-anchor: none;
}
.track-spacer {
    position: relative;
    width: 100%;
}
.track-pooled {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    will-change: transform;
}
</style>

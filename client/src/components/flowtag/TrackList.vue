<template>
<div ref='viewport' class='track-scroll' @scroll.passive='onScroll'>
    <div class='track-spacer' :style='{ height: visible.length * ROW_H + "px" }'>
        <TrackRow
            v-for='row in rows'
            :key='row.slot'
            class='track-pooled'
            :style='{ transform: "translateY(" + row.index * ROW_H + "px)" }'
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
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { get1t } from '../../scripts/onetagger';
import { getTagStore, FlowTrack } from '../../scripts/tagstore';
import TrackRow from './TrackRow.vue';

/** Fixed row height: every position is index * ROW_H */
const ROW_H = 40;
/** Rows rendered beyond each edge of the viewport */
const OVERSCAN = 8;

const $1t = get1t();
const store = getTagStore();
const emit = defineEmits<{ (e: 'play', track: FlowTrack): void }>();

const viewport = ref<HTMLElement>();
const scrollTop = ref(0);
const viewportH = ref(800);

// The actively playing file (may differ from the anchor with continue-playback)
const playerPath = computed(() => $1t.player.value.path);

// Recomputes only when tracks / filter / toggle change
const visible = computed(() => {
    // Touch reactive deps explicitly so the computed tracks them
    store.filter.value;
    store.untaggedOnly.value;
    return store.visibleTracks();
});

interface PooledRow { slot: number; index: number; track: FlowTrack }

/**
 * The rendered window, ordered by pool slot. Row i always lives in slot
 * i % poolSize, and the slot order never changes, so scrolling reuses the
 * same components in place: no mounts, no unmounts, no DOM moves, only the
 * props of slots whose track changed are patched.
 */
const rows = computed<PooledRow[]>(() => {
    const list = visible.value;
    const poolSize = Math.ceil(viewportH.value / ROW_H) + OVERSCAN * 2;
    // Bounded by the list so a stale scroll position can never render nothing
    const first = Math.min(
        Math.max(0, Math.floor(scrollTop.value / ROW_H) - OVERSCAN),
        Math.max(0, list.length - poolSize),
    );
    const last = Math.min(list.length, first + poolSize);
    const bySlot: (PooledRow | undefined)[] = new Array(poolSize);
    for (let i = first; i < last; i++) {
        bySlot[i % poolSize] = { slot: i % poolSize, index: i, track: list[i] };
    }
    return bySlot.filter((r): r is PooledRow => r !== undefined);
});

// Scroll events already fire at most once per frame; the re-render is
// batched before paint, so the window never lags the scroll position
function onScroll() {
    scrollTop.value = viewport.value?.scrollTop ?? 0;
}

let resizeObserver: ResizeObserver | undefined;
onMounted(() => {
    const el = viewport.value;
    if (!el) return;
    viewportH.value = el.clientHeight || viewportH.value;
    resizeObserver = new ResizeObserver(() => {
        viewportH.value = el.clientHeight || viewportH.value;
    });
    resizeObserver.observe(el);
});
onUnmounted(() => resizeObserver?.disconnect());

// A shrinking list leaves scrollTop past the end; the browser doesn't always
// clamp it, so clamp explicitly once the spacer has its new height
watch(() => visible.value.length, (len) => {
    nextTick(() => {
        const el = viewport.value;
        if (!el) return;
        const max = Math.max(0, len * ROW_H - el.clientHeight);
        if (el.scrollTop > max) el.scrollTop = max;
        scrollTop.value = Math.min(el.scrollTop, max);
    });
});

// Switching scope (playlist / source / collection) starts at the top
watch([store.activePlaylistId, store.activeSourceId], () => {
    nextTick(() => {
        if (viewport.value) viewport.value.scrollTop = 0;
        scrollTop.value = 0;
    });
});

function select(track: FlowTrack, ev: MouseEvent) {
    store.clickSelect(track, {
        toggle: ev.metaKey || ev.ctrlKey,
        range: ev.shiftKey,
    });
}

function scrollToIndex(idx: number, align: 'start' | 'end' | 'center') {
    const el = viewport.value;
    if (!el) return;
    const top = idx * ROW_H;
    const target = align === 'start'
        ? top
        : align === 'end'
            ? top + ROW_H - el.clientHeight
            : top - (el.clientHeight - ROW_H) / 2;
    el.scrollTop = Math.max(0, target);
    scrollTop.value = el.scrollTop;
}

// Keep the keyboard-selected row in view with the minimal scroll: already
// visible does nothing, one step off the edge nudges by a row (native-list
// cruise while holding J/K), and only a far jump (D to next untagged,
// filter change) recenters.
watch(store.selectedId, (id) => {
    if (id === undefined) return;
    const idx = visible.value.findIndex((t) => t.id === id);
    if (idx === -1) return;
    const el = viewport.value;
    if (!el) return;
    const top = idx * ROW_H;
    const bottom = top + ROW_H;
    const viewTop = el.scrollTop;
    const viewBottom = viewTop + el.clientHeight;
    if (top >= viewTop && bottom <= viewBottom) return;
    const far = bottom < viewTop - el.clientHeight || top > viewBottom + el.clientHeight;
    scrollToIndex(idx, far ? 'center' : (top < viewTop ? 'start' : 'end'));
});
</script>

<style lang='scss'>
// Unscoped: the pooled class lands on TrackRow's root element
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

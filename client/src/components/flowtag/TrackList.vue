<template>
<ContextMenuRoot>
<ContextMenuTrigger as-child>
<div ref='viewport' class='track-scroll' @contextmenu='onContextMenu'>
    <div class='track-spacer' :style='{ height: virtualizer.getTotalSize() + "px" }'>
        <TrackRow
            v-for='row in rows'
            v-show='!row.hidden'
            :key='row.slot'
            class='track-pooled'
            :style='{ transform: "translateY(" + row.start + "px)" }'
            :track='row.track'
            :data-track-id='row.track.id'
            :selected='store.isSelected(row.track.id)'
            :anchor='row.track.id === store.selectedId.value'
            :alt='row.index % 2 === 1'
            :playing='row.track.path === playerPath'
            :chip-max='narrowViewport ? 1 : undefined'
            @click='(ev) => select(row.track, ev)'
            @play='emit("play", row.track)'
        />
    </div>
</div>
</ContextMenuTrigger>
<ContextMenuPortal>
    <ContextMenuContent class='flowtag-theme ft-ctx' :side-offset='4'>
        <ContextMenuItem class='ft-ctx-item' :disabled='!menuHasTarget' data-test='ctx-stage-toggle' @select='store.toggleStageSelection()'>
            {{ menuAllStaged ? 'Unstage' : 'Stage' }}{{ menuCount > 1 ? ` ${menuCount} tracks` : '' }}
        </ContextMenuItem>
    </ContextMenuContent>
</ContextMenuPortal>
</ContextMenuRoot>
</template>

<script lang='ts' setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useVirtualizer } from '@tanstack/vue-virtual';
import { useMediaQuery } from '@vueuse/core';
import { ContextMenuContent, ContextMenuItem, ContextMenuPortal, ContextMenuRoot, ContextMenuTrigger } from 'reka-ui';
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
/** Narrow windows cap each row at one chip + "+n" (full list on its tooltip) */
const narrowViewport = useMediaQuery('(max-width: 899px)');

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

// ---- Right-click: Stage / Unstage on the selection ----
const menuHasTarget = ref(false);
const menuCount = computed(() => store.allSelected().length);
const menuAllStaged = computed(() => {
    store.tracks.value;
    const ids = new Set(store.allSelected());
    const picked = store.tracks.value.filter((t) => ids.has(t.id));
    return picked.length > 0 && picked.every((t) => t.staged);
});

/** The row under the cursor joins the selection (unless it is already in it) */
function onContextMenu(ev: MouseEvent) {
    const el = (ev.target as HTMLElement | null)?.closest<HTMLElement>('[data-track-id]');
    const id = el ? Number(el.dataset.trackId) : NaN;
    menuHasTarget.value = Number.isFinite(id);
    if (!Number.isFinite(id) || store.isSelected(id)) return;
    const track = visible.value.find((t) => t.id === id);
    if (track) store.clickSelect(track);
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
.ft-ctx {
    z-index: 3100;
    min-width: 8rem;
    padding: 4px;
    border-radius: 8px;
    border: 1px solid var(--ft-line-strong);
    background: var(--ft-overlay);
    color: var(--ft-text);
    box-shadow: var(--ft-shadow-pop);
}
.ft-ctx-item {
    display: flex;
    align-items: center;
    height: 28px;
    padding: 0 8px;
    border-radius: 6px;
    font-size: 13px;
    color: var(--ft-text-2);
    outline: none;
    cursor: default;
    user-select: none;
    &[data-highlighted] { background: var(--ft-hover); color: var(--ft-text); }
    &[data-disabled] { opacity: 0.5; pointer-events: none; }
}
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

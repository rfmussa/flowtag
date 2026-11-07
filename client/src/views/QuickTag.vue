<template>
<div class='flowtag flowtag-theme column no-wrap'>

    <!-- Header -->
    <div class='header-bar row items-center no-wrap'>
        <FolderManager />
        <ScanStatus class='q-ml-sm' />

        <div class='seg q-ml-md' role='group' aria-label='Scope'>
            <button type='button' class='seg-btn' :class='{on: !store.untaggedOnly.value}' @click='setUntagged(false, $event)'>All</button>
            <button type='button' class='seg-btn' :class='{on: store.untaggedOnly.value}' @click='setUntagged(true, $event)'>Untagged</button>
        </div>

        <q-select
            v-model='store.sortBy.value'
            :options='sortOptions'
            dense options-dense borderless emit-value map-options
            class='q-ml-sm sort-select'
            popup-content-class='flowtag-theme ft-pop'
        />
        <button type='button' class='ft-btn dir-btn' :aria-label='store.sortDesc.value ? "Sort descending" : "Sort ascending"' @click='toggleSortDir'>
            <q-icon :name='store.sortDesc.value ? "mdi-arrow-down-thin" : "mdi-arrow-up-thin"' size='15px' />
        </button>

        <!-- Active tag filters -->
        <template v-if='store.filterTagIds.value.length > 0'>
            <span v-for='tagId in store.filterTagIds.value' :key='tagId' class='flowtag-chip filter-chip q-ml-sm'>
                <span class='chip-dot' :style='{ background: store.tagById(tagId)?.color }'></span>
                <span class='chip-name'>{{ store.tagById(tagId)?.name }}</span>
                <button type='button' class='chip-x' :aria-label='`Stop filtering by ${store.tagById(tagId)?.name}`' @click='store.toggleTagFilter(tagId)'>×</button>
            </span>
        </template>

        <q-space />

        <span class='counts ft-num'>{{ taggedCount.toLocaleString() }} <span class='counts-dim'>/ {{ store.tracks.value.length.toLocaleString() }} tagged</span></span>
        <FlushIndicator class='q-ml-md' />

        <label class='ft-field search-field q-ml-md'>
            <q-icon name='mdi-magnify' size='14px' class='search-icon' />
            <input ref='filterEl' v-model='filterInput' type='text' placeholder='Filter tracks or tags' spellcheck='false' @keydown.escape.prevent='blurFilter' />
            <span v-if='!filterInput' class='ft-kbd'>F</span>
        </label>

        <button type='button' class='ft-btn q-ml-sm' @click='openManager'>Manage</button>
        <button type='button' class='ft-btn' @click='confirmExport' title="Write tags as #hashtags into each file's comment (Rekordbox-searchable)">Export</button>
        <button type='button' class='ft-btn icon-only' :aria-label='paletteOpen ? "Hide tag palette" : "Show tag palette"' @click='togglePalette'>
            <q-icon :name='paletteOpen ? "mdi-dock-right" : "mdi-dock-left"' size='15px' />
        </button>
    </div>

    <!-- Session progress: one quiet line -->
    <div class='progress-line' aria-hidden='true'><div class='progress-fill' :style='{ width: progressPct + "%" }'></div></div>

    <!-- Track list + palette -->
    <div class='col row no-wrap list-row'>
        <div class='col list-area'>
            <div v-if='store.folders.value.length === 0 && store.loaded.value' class='column items-center justify-center full-height empty-state'>
                <q-icon name='mdi-music-box-multiple-outline' size='44px' class='empty-icon' />
                <div class='empty-title'>No music folders yet</div>
                <div class='empty-sub'>Add a folder — a 20,000-track library scans in seconds.</div>
                <FolderManager class='q-mt-md' />
            </div>
            <div v-else-if='!store.loaded.value' class='column items-center justify-center full-height'>
                <q-spinner size='28px' color='grey-6' />
            </div>
            <TrackList v-else />
        </div>
        <div v-if='paletteOpen' class='palette-col'>
            <TagPalette />
        </div>
    </div>

    <!-- Tagging panel -->
    <div class='tag-panel row items-center no-wrap'>
        <template v-if='selected'>
            <div class='panel-meta'>
                <span class='panel-title' :title='selected.title'>{{ selected.title }}</span>
                <span class='panel-artist' :title='selected.artist'>{{ selected.artist }}</span>
            </div>
            <span v-if='multiCount > 1' class='multi-badge ft-num'>
                {{ multiCount }} tracks
                <q-tooltip>Tag edits apply to all selected tracks — Esc clears</q-tooltip>
            </span>
            <TagChips :tag-ids='selected.tagIds' :track-id='selected.id' removable class='panel-chips' />
            <TagInput :track-id='selected.id' />
        </template>
        <div v-else class='panel-hint'>
            <span class='ft-kbd'>J</span><span class='ft-kbd'>K</span> select
            <span class='hint-sep'></span>
            <span class='ft-kbd'>T</span> tag
            <span class='hint-sep'></span>
            <span class='ft-kbd'>D</span> done &amp; next
            <span class='hint-sep'></span>
            <span class='ft-kbd'>Space</span> play
        </div>
    </div>

    <TagManager v-model='managerOpen' />
</div>
</template>

<script lang='ts' setup>
import { computed, onMounted, ref, watch } from 'vue';
import { Dialog, Notify } from 'quasar';
import { get1t } from '../scripts/onetagger';
import { getTagStore } from '../scripts/tagstore';
import FolderManager from '../components/flowtag/FolderManager.vue';
import FlushIndicator from '../components/flowtag/FlushIndicator.vue';
import ScanStatus from '../components/flowtag/ScanStatus.vue';
import TrackList from '../components/flowtag/TrackList.vue';
import TagChips from '../components/flowtag/TagChips.vue';
import TagInput from '../components/flowtag/TagInput.vue';
import TagPalette from '../components/flowtag/TagPalette.vue';
import TagManager from '../components/flowtag/TagManager.vue';

const PALETTE_KEY = 'flowtag.paletteOpen';

const $1t = get1t();
const store = getTagStore();

const selected = computed(() => {
    store.tracks.value; // reactive dep
    return store.selectedTrack();
});
const multiCount = computed(() => store.allSelected().length);
const taggedCount = computed(() => store.tracks.value.reduce((n, t) => n + (t.tagIds.length > 0 ? 1 : 0), 0));
const progressPct = computed(() => {
    const total = store.tracks.value.length;
    return total === 0 ? 0 : Math.round((taggedCount.value / total) * 1000) / 10;
});

// Debounced filter binding (the old q-input had debounce built in)
const filterInput = ref(store.filter.value);
let filterTimer: any;
watch(filterInput, (v) => {
    clearTimeout(filterTimer);
    filterTimer = setTimeout(() => { store.filter.value = v ?? ''; }, 60);
});

// F focuses the filter (from the global keyboard map)
const filterEl = ref<HTMLInputElement>();
watch(store.focusFilterSignal, () => filterEl.value?.focus());
function blurFilter() {
    filterEl.value?.blur();
}

const managerOpen = ref(false);
const paletteOpen = ref(readPaletteOpen());

const sortOptions = [
    { label: 'Path', value: 'path' },
    { label: 'Title', value: 'title' },
    { label: 'Artist', value: 'artist' },
    { label: 'BPM', value: 'bpm' },
    { label: 'Key', value: 'key' },
    { label: 'Year', value: 'year' },
];

function readPaletteOpen(): boolean {
    try {
        return localStorage.getItem(PALETTE_KEY) !== 'false';
    } catch {
        return true;
    }
}

function blurTarget(e: Event) {
    (e.currentTarget as HTMLElement | null)?.blur();
}

function setUntagged(v: boolean, e: Event) {
    blurTarget(e);
    store.untaggedOnly.value = v;
}

function toggleSortDir(e: Event) {
    blurTarget(e);
    store.sortDesc.value = !store.sortDesc.value;
}

function openManager(e: Event) {
    blurTarget(e);
    managerOpen.value = true;
}

function confirmExport(e: Event) {
    blurTarget(e);
    Dialog.create({
        title: 'Export to comments',
        message: 'Append your tags as #hashtags to every tagged file\'s comment field (searchable in Rekordbox). Existing comment text is kept; re-exporting updates the hashtag block. Continue?',
        cancel: true,
        dark: true,
    }).onOk(() => {
        $1t.send('rekordboxExport', { mode: 'comment' });
        Notify.create({ message: 'Export started…', timeout: 1500 });
    });
}

function togglePalette(e: Event) {
    blurTarget(e);
    paletteOpen.value = !paletteOpen.value;
    try {
        localStorage.setItem(PALETTE_KEY, String(paletteOpen.value));
    } catch {
        // Storage unavailable: keep the in-memory state only
    }
}

// Selecting a track loads it into the player (playback from 0);
// playback continues across switches when it was already running
watch(store.selectedId, (id, previousId) => {
    const track = store.selectedTrack();
    if (id === undefined || !track || track.missing) return;
    const player = $1t.player.value;
    const autoplay = player.playing || player.wasPlaying;
    player.startTrack(track.path, track.id, track.title, track.artist, autoplay);
    // Expedite only the track just left (not every pending write at once)
    if (previousId !== undefined) {
        $1t.send('flushHint', { trackId: previousId });
    }
});

onMounted(() => {
    if (!store.loaded.value) store.load();
    $1t.send('flushStatusGet');
});
</script>

<style lang='scss'>
.flowtag {
    height: calc(100vh - 100px);
    background: var(--ft-bg);

    // Quiet the Quasar controls that remain inside the theme
    .q-btn { color: var(--ft-text-2); }
    .q-field--borderless .q-field__control { color: var(--ft-text-2); font-size: 12.5px; }
}

.flowtag .header-bar {
    height: 46px;
    box-sizing: border-box;
    padding: 0 12px;
    border-bottom: 1px solid var(--ft-line);
    background: var(--ft-bg);
    flex-shrink: 0;
}

.flowtag .seg {
    display: inline-flex;
    background: var(--ft-panel);
    border: 1px solid var(--ft-line);
    border-radius: 6px;
    padding: 2px;

    .seg-btn {
        border: none;
        background: transparent;
        color: var(--ft-text-3);
        font-size: 12px;
        font-weight: 500;
        font-family: inherit;
        padding: 3px 10px;
        border-radius: 4px;
        cursor: pointer;
        &:hover { color: var(--ft-text-2); }
        &.on { background: var(--ft-raised); color: var(--ft-text); }
    }
}

.flowtag .sort-select {
    min-width: 72px;
    .q-field__native { color: var(--ft-text-2); }
}
.flowtag .dir-btn { padding: 0 5px; height: 26px; }
.flowtag .icon-only { padding: 0 6px; }

.flowtag .filter-chip { height: 24px; }

.flowtag .counts {
    font-size: 12px;
    color: var(--ft-text-2);
    .counts-dim { color: var(--ft-text-3); }
}

.flowtag .search-field {
    width: 230px;
    .search-icon { color: var(--ft-text-3); }
}

.flowtag .progress-line {
    height: 2px;
    background: var(--ft-panel);
    flex-shrink: 0;
    .progress-fill {
        height: 100%;
        background: var(--ft-text-3);
        transition: width 300ms ease-out;
    }
}

.flowtag .list-row { min-height: 0; }
.flowtag .list-area { min-height: 0; min-width: 0; overflow: hidden; }

.flowtag .palette-col {
    width: 240px;
    flex-shrink: 0;
    border-left: 1px solid var(--ft-line);
    background: var(--ft-panel);
    min-height: 0;
    overflow: hidden;
}

.flowtag .empty-state {
    .empty-icon { color: var(--ft-text-3); margin-bottom: 14px; }
    .empty-title { font-size: 15px; font-weight: 600; color: var(--ft-text); }
    .empty-sub { font-size: 12.5px; color: var(--ft-text-3); margin-top: 4px; }
}

.flowtag .tag-panel {
    border-top: 1px solid var(--ft-line);
    background: var(--ft-bg);
    flex-shrink: 0;
    min-height: 52px;
    box-sizing: border-box;
    padding: 0 16px;
    gap: 12px;

    .panel-meta {
        display: flex;
        align-items: baseline;
        gap: 8px;
        min-width: 120px;
        max-width: 300px;
        overflow: hidden;
        .panel-title { font-size: 13px; font-weight: 600; color: var(--ft-text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
        .panel-artist { font-size: 12px; color: var(--ft-text-3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; flex-shrink: 0; max-width: 45%; }
    }
    .multi-badge {
        font-size: 11.5px;
        font-weight: 600;
        color: var(--ft-text);
        background: var(--ft-raised);
        border: 1px solid var(--ft-line-strong);
        border-radius: 5px;
        padding: 2px 8px;
        flex-shrink: 0;
    }
    .panel-chips { max-width: 38%; overflow: hidden; }
    .panel-hint {
        display: flex;
        align-items: center;
        gap: 6px;
        width: 100%;
        justify-content: center;
        font-size: 12px;
        color: var(--ft-text-3);
        .hint-sep { width: 14px; }
    }
}
.flowtag .full-height { height: 100%; }
</style>

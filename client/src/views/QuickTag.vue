<template>
<div class='flowtag flowtag-theme flex flex-col'>

    <!-- Header -->
    <div class='header-bar flex items-center'>
        <FolderManager />
        <ScanStatus class='ml-2' />

        <div class='seg ml-4' role='group' aria-label='Scope'>
            <button type='button' class='seg-btn' :class='{on: !store.untaggedOnly.value}' @click='setUntagged(false, $event)'>All</button>
            <button type='button' class='seg-btn' :class='{on: store.untaggedOnly.value}' @click='setUntagged(true, $event)'>Untagged</button>
        </div>

        <OptionSelect
            v-model='store.sortBy.value'
            :options='sortOptions'
            label='Sort by'
            trigger-class='ml-2 border-transparent bg-transparent hover:bg-ft-hover hover:border-transparent text-ft-text-2 font-medium h-[26px] pl-2.5 pr-1.5'
        />
        <button type='button' class='ft-btn dir-btn' :aria-label='store.sortDesc.value ? "Sort descending" : "Sort ascending"' @click='toggleSortDir'>
            <ArrowDownIcon v-if='store.sortDesc.value' class='size-[15px]' />
            <ArrowUpIcon v-else class='size-[15px]' />
        </button>

        <!-- Active tag filters -->
        <template v-if='store.filterTagIds.value.length > 0'>
            <span v-for='tagId in store.filterTagIds.value' :key='tagId' class='flowtag-chip filter-chip ml-2'>
                <span class='chip-dot' :style='{ background: store.tagById(tagId)?.color }'></span>
                <span class='chip-name'>{{ store.tagById(tagId)?.name }}</span>
                <button type='button' class='chip-x' :aria-label='`Stop filtering by ${store.tagById(tagId)?.name}`' @click='store.toggleTagFilter(tagId)'>×</button>
            </span>
        </template>

        <div class='flex-1'></div>

        <span class='counts ft-num'>{{ taggedCount.toLocaleString() }} <span class='counts-dim'>/ {{ store.tracks.value.length.toLocaleString() }} tagged</span></span>
        <FlushIndicator class='ml-4' />

        <label class='ft-field search-field ml-4'>
            <SearchIcon class='search-icon size-3.5 shrink-0' />
            <input ref='filterEl' v-model='filterInput' type='text' placeholder='Filter tracks or tags' spellcheck='false' @keydown.escape.prevent='blurFilter' />
            <span v-if='!filterInput' class='ft-kbd'>F</span>
        </label>

        <button type='button' class='ft-btn ml-2' @click='openManager'>Manage</button>
        <button type='button' class='ft-btn' @click='confirmExport' title="Write tags as #hashtags into each file's comment (Rekordbox-searchable)">Export</button>
        <button type='button' class='ft-btn icon-only' :aria-label='paletteOpen ? "Hide tag palette" : "Show tag palette"' @click='togglePalette'>
            <PanelRightCloseIcon v-if='paletteOpen' class='size-[15px]' />
            <PanelRightOpenIcon v-else class='size-[15px]' />
        </button>
        <button v-if='hasShell' type='button' class='ft-btn icon-only' aria-label='Toggle miniplayer' @click='toggleMini'>
            <PictureInPicture2Icon class='size-[15px]' />
        </button>
        <button type='button' class='ft-btn icon-only' aria-label='Settings' @click='openSettings'>
            <SettingsIcon class='size-[15px]' />
        </button>
    </div>

    <!-- Session progress: one quiet line -->
    <div class='progress-line' aria-hidden='true'><div class='progress-fill' :style='{ width: progressPct + "%" }'></div></div>

    <!-- Playlists + track list + palette -->
    <div class='flex flex-1 list-row'>
        <PlaylistSidebar v-if='sidebarOpen' />
        <div class='flex-1 list-area'>
            <div v-if='store.folders.value.length === 0 && store.loaded.value' class='flex flex-col items-center justify-center h-full empty-state'>
                <LibraryIcon class='empty-icon size-11' :stroke-width='1.25' />
                <div class='empty-title'>No music folders yet</div>
                <div class='empty-sub'>Add a folder — a 20,000-track library scans in seconds.</div>
                <FolderManager class='mt-4' />
            </div>
            <div v-else-if='!store.loaded.value' class='flex items-center justify-center h-full'>
                <LoaderCircleIcon class='size-7 animate-spin text-ft-text-3' aria-label='Loading library' />
            </div>
            <TrackList v-else @play='playTrack' />
        </div>
        <div v-if='paletteOpen' class='palette-col'>
            <TagPalette />
        </div>
    </div>

    <!-- Tagging panel -->
    <div class='tag-panel flex items-center'>
        <template v-if='selected'>
            <div class='panel-meta'>
                <span class='panel-title' :title='selected.title'>{{ selected.title }}</span>
                <span class='panel-artist' :title='selected.artist'>{{ selected.artist }}</span>
            </div>
            <Hint v-if='multiCount > 1' text='Tag edits apply to all selected tracks — Esc clears'>
                <span class='multi-badge ft-num'>{{ multiCount }} tracks</span>
            </Hint>
            <TagChips :tag-ids='selected.tagIds' :track-id='selected.id' removable class='panel-chips' />
            <TagInput :track-id='selected.id' />
            <button
                v-if='activeManualPlaylist'
                type='button' class='ft-btn icon-only'
                :title='"Remove from " + activeManualPlaylist.name'
                @click='removeFromPlaylist'
            >
                <ListMinusIcon class='size-4' />
            </button>
        </template>
        <div v-else class='panel-hint'>
            <span class='ft-kbd'>J</span><span class='ft-kbd'>K</span> select
            <span class='hint-sep'></span>
            <span class='ft-kbd'>T</span> tag
            <span class='hint-sep'></span>
            <span class='ft-kbd'>D</span> done &amp; next
            <span class='hint-sep'></span>
            <span class='ft-kbd'>E</span> edit
            <span class='hint-sep'></span>
            <span class='ft-kbd'>M</span> lookup
            <span class='hint-sep'></span>
            <span class='ft-kbd'>B</span> playlists
            <span class='hint-sep'></span>
            <span class='ft-kbd'>Space</span> play
        </div>
    </div>

    <ExportDialog v-model='exportOpen' />
    <TagManager v-model='managerOpen' />
    <TrackInspector />
    <ManualTag :path='manualTagPath' @exit='onManualTagExit' />
</div>
</template>

<script lang='ts' setup>
import { computed, onMounted, ref, watch } from 'vue';
import { useLocalStorage, watchDebounced } from '@vueuse/core';
import {
    ArrowDownIcon, ArrowUpIcon, LibraryIcon, ListMinusIcon, LoaderCircleIcon,
    PanelRightCloseIcon, PanelRightOpenIcon, PictureInPicture2Icon, SearchIcon, SettingsIcon,
} from '@lucide/vue';
import { get1t } from '../scripts/onetagger';
import { getTagStore, FlowTrack, SortField } from '../scripts/tagstore';
import { diag } from '../scripts/diag';
import FolderManager from '../components/flowtag/FolderManager.vue';
import FlushIndicator from '../components/flowtag/FlushIndicator.vue';
import ScanStatus from '../components/flowtag/ScanStatus.vue';
import TrackList from '../components/flowtag/TrackList.vue';
import PlaylistSidebar from '../components/flowtag/PlaylistSidebar.vue';
import ExportDialog from '../components/flowtag/ExportDialog.vue';
import TagChips from '../components/flowtag/TagChips.vue';
import TagInput from '../components/flowtag/TagInput.vue';
import TagPalette from '../components/flowtag/TagPalette.vue';
import OptionSelect from '../components/flowtag/OptionSelect.vue';
import Hint from '../components/flowtag/Hint.vue';
import TagManager from '../components/flowtag/TagManager.vue';
import TrackInspector from '../components/flowtag/TrackInspector.vue';
import ManualTag from '../components/ManualTag.vue';

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
watchDebounced(filterInput, (v) => { store.filter.value = v ?? ''; }, { debounce: 60 });
// The store can set the filter too (selecting a smart playlist): mirror back
watch(store.filter, (v) => {
    if (v !== filterInput.value) filterInput.value = v;
});

// F focuses the filter (from the global keyboard map)
const filterEl = ref<HTMLInputElement>();
watch(store.focusFilterSignal, () => filterEl.value?.focus());
function blurFilter() {
    filterEl.value?.blur();
}

const managerOpen = ref(false);
const paletteOpen = useLocalStorage(PALETTE_KEY, true);

const sortOptions: { label: string; value: SortField }[] = [
    { label: 'Path', value: 'path' },
    { label: 'Title', value: 'title' },
    { label: 'Artist', value: 'artist' },
    { label: 'BPM', value: 'bpm' },
    { label: 'Key', value: 'key' },
    { label: 'Year', value: 'year' },
    { label: 'Added', value: 'added' },
];


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
    exportOpen.value = true;
}
const exportOpen = ref(false);

// Playlists sidebar: persisted like the palette, toggled by the B action
const sidebarOpen = useLocalStorage('flowtag-sidebar', false);
watch(store.sidebarSignal, () => {
    sidebarOpen.value = !sidebarOpen.value;
});

// Remove the selection from the active (manual) playlist
const activeManualPlaylist = computed(() => {
    const pl = store.activePlaylist();
    return pl && !pl.filter ? pl : undefined;
});
function removeFromPlaylist(e: Event) {
    blurTarget(e);
    const pl = activeManualPlaylist.value;
    if (!pl) return;
    const trackIds = store.allSelected();
    if (trackIds.length === 0) return;
    $1t.send('playlistRemoveTracks', { id: pl.id, trackIds });
    diag('playlistRemoveTracks', { id: pl.id, tracks: trackIds.length });
}

// Settings dialog lives app-level (App.vue); ask it to open
function openSettings(e: Event) {
    blurTarget(e);
    window.dispatchEvent(new CustomEvent('flowtag-open-settings'));
}

// Floating miniplayer: only the native shell can spawn windows
// @ts-ignore
const hasShell = typeof window.ipc !== 'undefined';
function toggleMini(e: Event) {
    blurTarget(e);
    try {
        // @ts-ignore
        window.ipc.postMessage('toggleMini');
    } catch (err) { /* browser mode */ }
}

// M: online metadata lookup for the selected track (Beatport/Discogs/...).
// Playback stops first so an eventual Apply can write the file.
const manualTagPath = ref<string | undefined>();
watch(store.manualTagSignal, () => {
    const track = store.selectedTrack();
    if (!track || track.missing) return;
    $1t.player.value.stop();
    diag('lookupOpen', { trackId: track.id });
    manualTagPath.value = track.path;
});
function onManualTagExit() {
    diag('lookupClosed');
    manualTagPath.value = undefined;
    // Pick up whatever the lookup wrote (mtime diff -> only that file re-parses)
    $1t.send('tagLibraryScan', { full: false });
}

function togglePalette(e: Event) {
    blurTarget(e);
    paletteOpen.value = !paletteOpen.value;
}

// Selection is pure by default: browsing never starts or switches playback.
// The player follows selection only when asked explicitly (wasPlaying, set by
// D / media next/prev) or when the play-on-select setting is on.
watch(store.selectedId, (id, previousId) => {
    const track = store.selectedTrack();
    if (id === undefined || !track || track.missing) return;
    const player = $1t.player.value;
    const explicit = player.wasPlaying;
    if (explicit || $1t.settings.value.playOnSelect) {
        const autoplay = player.playing || player.wasPlaying;
        player.startTrack(track.path, track.id, track.title, track.artist, autoplay);
    }
    // Expedite only the track just left (not every pending write at once)
    if (previousId !== undefined) {
        $1t.send('flushHint', { trackId: previousId });
    }
});

// Double-click always plays that row
function playTrack(track: FlowTrack) {
    $1t.player.value.startTrack(track.path, track.id, track.title, track.artist, true);
}

onMounted(() => {
    if (!store.loaded.value) store.load();
    $1t.send('flushStatusGet');
});
</script>

<style lang='scss'>
.flowtag {
    // Fills App's main area (everything above the docked player)
    height: 100%;
    background: var(--ft-bg);
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
        // Left-aligned like everything else on the surface (rows, header)
        justify-content: flex-start;
        padding-left: 6px;
        font-size: 12px;
        color: var(--ft-text-3);
        .hint-sep { width: 14px; }
    }
}
</style>

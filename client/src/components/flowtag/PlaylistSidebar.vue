<template>
<div class='pl-sidebar'>
    <!-- Stage: pinned while it has tracks -->
    <div
        v-if='store.stageTotal.value > 0'
        class='pl-row pl-stage'
        :class='{ active: store.scope.value === "stage" }'
        data-test='stage-entry'
        @click='store.selectStage()'
    >
        <LayersIcon class='pl-icon' />
        <span class='pl-name'>Stage</span>
        <span class='pl-count ft-num' data-test='stage-count'>{{ stageCountLabel(store.stageReady.value, store.stageTotal.value) }}</span>
        <Button size='sm' variant='ghost' class='stage-commit' tabindex='-1' data-test='stage-commit' @click.stop='openCommitSheet()'>Commit…</Button>
        <div class='stage-bar' aria-hidden='true'><div class='stage-bar-fill' :style='{ width: stagePct + "%" }'></div></div>
    </div>

    <!-- Collection (everything) -->
    <div
        class='pl-row'
        :class='{ active: store.scope.value === "all" && store.activeSourceId.value === undefined }'
        @click='select(undefined)'
    >
        <LibraryIcon class='pl-icon' />
        <span class='pl-name'>Collection</span>
        <span class='pl-count ft-num'>{{ store.tracks.value.length.toLocaleString() }}</span>
    </div>

    <!-- One scope per source folder -->
    <template v-if='store.folders.value.length > 0'>
        <div class='pl-head pl-head-toggle' @click='toggleSection("sources")'>
            <span class='ft-label head-label'>
                <ChevronDownIcon v-if='sourcesOpen' class='head-chevron' /><ChevronRightIcon v-else class='head-chevron' />
                Sources
            </span>
            <span class='pl-head-actions' @click.stop>
                <button type='button' class='ft-btn icon-only' title='Add source folder' @click='addSource'>
                    <PlusIcon class='size-[15px]' />
                </button>
            </span>
        </div>
        <template v-if='sourcesOpen'>
        <div
            v-for='f in store.folders.value'
            :key='"src" + f.id'
            class='pl-row'
            :class='{ active: store.scope.value !== "stage" && store.activeSourceId.value === f.id }'
            :title='f.path'
            @click='store.selectSource(f.id)'
        >
            <FolderIcon class='pl-icon' />
            <template v-if='editing?.kind === "source" && editing.id === f.id'>
                <input
                    class='pl-rename' :value='f.name' spellcheck='false'
                    @keydown.enter='commitRename(($event.target as HTMLInputElement).value)'
                    @keydown.escape='editing = undefined'
                    @blur='commitRename(($event.target as HTMLInputElement).value)'
                    @click.stop
                    v-focus
                />
            </template>
            <span v-else class='pl-name' @dblclick.stop='startRename("source", f.id)'>{{ f.name }}</span>
            <span class='pl-count ft-num'>{{ sourceCount(f.id).toLocaleString() }}</span>
        </div>
        </template>
    </template>

    <div class='pl-head pl-head-toggle' @click='toggleSection("playlists")'>
        <span class='ft-label head-label'>
            <ChevronDownIcon v-if='playlistsOpen' class='head-chevron' /><ChevronRightIcon v-else class='head-chevron' />
            Playlists
        </span>
        <span class='pl-head-actions' @click.stop>
            <button
                v-if='filtersActive'
                type='button' class='ft-btn icon-only'
                title='Save the current filter as a smart playlist'
                @click='saveFilterAsPlaylist'
            >
                <ListFilterPlusIcon class='size-3.5' />
            </button>
            <button type='button' class='ft-btn icon-only' title='New playlist' @click='createPlaylist'>
                <PlusIcon class='size-[15px]' />
            </button>
        </span>
    </div>

    <div v-if='playlistsOpen' class='pl-list'>
        <div
            v-for='pl in store.playlists.value'
            :key='pl.id'
            class='pl-row'
            :class='{ active: store.scope.value !== "stage" && store.activePlaylistId.value === pl.id, "drop-target": dropId === pl.id && !pl.filter }'
            @click='select(pl.id)'
            @dragover='onDragOver(pl, $event)'
            @dragleave='dropId = dropId === pl.id ? undefined : dropId'
            @drop='onDrop(pl, $event)'
        >
            <ListFilterIcon v-if='pl.filter' class='pl-icon' /><ListMusicIcon v-else class='pl-icon' />
            <template v-if='editing?.kind === "playlist" && editing.id === pl.id'>
                <input
                    class='pl-rename' :value='pl.name' spellcheck='false'
                    @keydown.enter='commitRename(($event.target as HTMLInputElement).value)'
                    @keydown.escape='editing = undefined'
                    @blur='commitRename(($event.target as HTMLInputElement).value)'
                    @click.stop
                    v-focus
                />
            </template>
            <span v-else class='pl-name' :title='pl.name' @dblclick.stop='startRename("playlist", pl.id)'>{{ pl.name }}</span>
            <span class='pl-count ft-num'>{{ playlistCount(pl).toLocaleString() }}</span>
            <DropdownMenu @update:open='(open: boolean) => open && ensureMembers(pl)'>
                <DropdownMenuTrigger as-child>
                    <button type='button' class='ft-btn icon-only pl-menu-btn' aria-label='Playlist menu' @click.stop>
                        <EllipsisIcon class='size-[13px]' />
                    </button>
                </DropdownMenuTrigger>
                <!-- Rename focuses its inline field; returning focus to the trigger would blur (commit) it -->
                <DropdownMenuContent align='end' class='min-w-[140px]' @close-auto-focus.prevent>
                    <DropdownMenuItem :disabled='stageablePlaylistIds(pl).length === 0' @select='stagePlaylist(pl)'>
                        Stage its not-done tracks ({{ stageablePlaylistIds(pl).length }})
                    </DropdownMenuItem>
                    <DropdownMenuItem v-if='pl.filter' @select='rulesFor = pl'>Edit rules</DropdownMenuItem>
                    <DropdownMenuItem @select='startRename("playlist", pl.id)'>Rename</DropdownMenuItem>
                    <DropdownMenuItem variant='destructive' @select='deletePlaylist(pl)'>Delete</DropdownMenuItem>
                </DropdownMenuContent>
            </DropdownMenu>
        </div>
        <div v-if='store.playlists.value.length === 0' class='pl-empty'>
            Drag tracks here, or save a filter as a playlist.
        </div>
    </div>

    <!-- Committed batches: one dated playlist per commit, for practicing -->
    <template v-if='commits.length > 0'>
        <div class='pl-head pl-head-toggle' @click='toggleSection("commits")'>
            <span class='ft-label head-label'>
                <ChevronDownIcon v-if='commitsOpen' class='head-chevron' /><ChevronRightIcon v-else class='head-chevron' />
                Commits
            </span>
        </div>
        <div v-if='commitsOpen' class='pl-list'>
            <div
                v-for='c in commits'
                :key='"c" + c.id'
                class='pl-row'
                :class='{ active: store.scope.value === commitScope(c.id) }'
                :title='commitTitle(c)'
                @click='store.selectCommit(c.id)'
            >
                <GitCommitHorizontalIcon class='pl-icon' />
                <span class='pl-name'>{{ commitLabel(c) }}</span>
                <span class='pl-count ft-num'>{{ c.trackCount.toLocaleString() }}</span>
            </div>
        </div>
    </template>

    <SmartPlaylistEditor :playlist='rulesFor' @close='rulesFor = undefined' />
</div>
</template>

<script lang='ts' setup>
import { computed, ref } from 'vue';
import { notify } from '@/lib/notify';
import { useLocalStorage } from '@vueuse/core';
import {
    ChevronDownIcon, ChevronRightIcon, EllipsisIcon, FolderIcon, GitCommitHorizontalIcon,
    LayersIcon, LibraryIcon, ListFilterIcon, ListFilterPlusIcon, ListMusicIcon, PlusIcon,
} from '@lucide/vue';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import { Button } from '@/components/ui/button';
import { confirmDialog, promptDialog } from '@/lib/dialogs';
import { commitLabel, commitScope, stageCountLabel } from '@/lib/commit';
import type { CommitRow } from '@/lib/journal';
import { openCommitSheet } from '../../scripts/commitsheet';
import { get1t } from '../../scripts/onetagger';
import { getTagStore, FlowPlaylist } from '../../scripts/tagstore';
import SmartPlaylistEditor from './SmartPlaylistEditor.vue';
import { diag } from '../../scripts/diag';

/** Tracks staged per "Stage its not-done tracks" action */
const STAGE_BATCH = 200;
const $1t = get1t();
const store = getTagStore();
const dropId = ref<number | undefined>(undefined);
const rulesFor = ref<FlowPlaylist | undefined>(undefined);
/** Inline rename target (double-click a name, or the menu's Rename) */
const editing = ref<{ kind: 'playlist' | 'source'; id: number } | undefined>(undefined);

const vFocus = { mounted: (el: HTMLInputElement) => { el.focus(); el.select(); } };

// Collapsible sections, remembered per machine
const sourcesOpen = useLocalStorage('flowtag-sec-sources', true);
const playlistsOpen = useLocalStorage('flowtag-sec-playlists', true);
const commitsOpen = useLocalStorage('flowtag-sec-commits', true);
function toggleSection(key: 'sources' | 'playlists' | 'commits') {
    const target = key === 'sources' ? sourcesOpen : key === 'playlists' ? playlistsOpen : commitsOpen;
    target.value = !target.value;
}

/** Committed batches, newest first (the journal loads on QuickTag mount) */
const commits = computed(() => store.journal.value?.commits ?? []);

function commitTitle(c: CommitRow): string {
    const when = new Date(c.createdAt * 1000).toLocaleString();
    const message = c.message.trim();
    return message ? `${when} — ${message}` : when;
}

// Same flow as the header's Folders menu (native or in-app picker) -> tagFolderAdd
function addSource() {
    diag('sourceAdd');
    $1t.browse('tagFolderAdd');
}

function startRename(kind: 'playlist' | 'source', id: number) {
    editing.value = { kind, id };
}

function commitRename(raw: string) {
    const target = editing.value;
    editing.value = undefined;
    if (!target) return;
    const name = raw.trim();
    if (!name) return;
    if (target.kind === 'playlist') {
        const pl = store.playlists.value.find((p) => p.id === target.id);
        if (pl && pl.name !== name) $1t.send('playlistRename', { id: target.id, name });
    } else {
        const f = store.folders.value.find((x) => x.id === target.id);
        if (f && f.name !== name) $1t.send('tagFolderRename', { id: target.id, name });
    }
    diag('inlineRename', { kind: target.kind, id: target.id });
}

/** Track counts per source folder, rebuilt when the library changes */
const sourceCounts = computed(() => {
    const counts = new Map<number, number>();
    for (const t of store.tracks.value) {
        counts.set(t.folderId, (counts.get(t.folderId) ?? 0) + 1);
    }
    return counts;
});
function sourceCount(id: number): number {
    return sourceCounts.value.get(id) ?? 0;
}

const filtersActive = computed(() =>
    store.filter.value.trim().length > 0
    || store.filterTagIds.value.length > 0
    || store.quickFilter.value !== 'all');

const stagePct = computed(() => {
    const total = store.stageTotal.value;
    return total === 0 ? 0 : Math.min(100, Math.round((store.stageReady.value / total) * 100));
});

/** Manual playlists only know their members once loaded; the menu needs them for its count */
function ensureMembers(pl: FlowPlaylist) {
    if (!pl.filter && !store.playlistMembers.value.has(pl.id)) $1t.send('playlistTracksLoad', { id: pl.id });
}

/** First 200 not-done, not-staged tracks of the playlist */
function stageablePlaylistIds(pl: FlowPlaylist): number[] {
    return store.notDoneUnstagedIds(store.playlistTrackIds(pl), STAGE_BATCH);
}

function stagePlaylist(pl: FlowPlaylist) {
    const ids = stageablePlaylistIds(pl);
    store.stageTrackIds(ids);
    diag('stagePlaylist', { id: pl.id, tracks: ids.length });
}

function select(id?: number) {
    if (id === undefined) store.selectSource(undefined);
    store.selectPlaylist(id);
}

/** Smart playlists count live; manual ones use the server's count */
/** Smart-playlist counts, evaluated once per library/playlist change instead
 *  of once per sidebar render (rule evaluation walks the whole library) */
const smartCounts = computed(() => {
    const counts = new Map<number, number>();
    for (const pl of store.playlists.value) {
        if (pl.filter) counts.set(pl.id, store.playlistTrackIds(pl).length);
    }
    return counts;
});

function playlistCount(pl: FlowPlaylist): number {
    return pl.filter ? (smartCounts.value.get(pl.id) ?? 0) : pl.count;
}

async function namePrompt(title: string, initial: string, onOk: (name: string) => void) {
    const name = await promptDialog({ title, value: initial, placeholder: 'Playlist name', okLabel: 'Save' });
    if (name) onOk(name);
}

function createPlaylist() {
    namePrompt('New playlist', '', (name) => {
        $1t.send('playlistCreate', { name, filter: null });
        diag('playlistCreate', { smart: false });
    });
}

function saveFilterAsPlaylist() {
    const saved = {
        text: store.filter.value.trim() || undefined,
        tagIds: store.filterTagIds.value.length ? [...store.filterTagIds.value] : undefined,
        untaggedOnly: store.quickFilter.value === 'untagged' || undefined,
        status: store.quickFilter.value === 'notdone' ? { is: 'notDone' } : undefined,
    };
    namePrompt('Save filter as playlist', '', (name) => {
        $1t.send('playlistCreate', { name, filter: JSON.stringify(saved) });
        diag('playlistCreate', { smart: true });
    });
}

async function deletePlaylist(pl: FlowPlaylist) {
    const ok = await confirmDialog({
        title: 'Delete playlist',
        message: pl.filter
            ? `Delete "${pl.name}"? The saved filter is lost; tracks are not touched.`
            : `Delete "${pl.name}"? Its ${pl.count} track(s) stay in the library.`,
        okLabel: 'Delete',
        destructive: true,
    });
    if (!ok) return;
    $1t.send('playlistDelete', { id: pl.id });
    diag('playlistDelete', { id: pl.id, smart: !!pl.filter });
}

// ---- Drag & drop from the track list ----
const MIME = 'application/x-flowtag-tracks';

function onDragOver(pl: FlowPlaylist, e: DragEvent) {
    if (pl.filter) return; // smart playlists take no manual members
    if (!e.dataTransfer?.types.includes(MIME)) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'copy';
    dropId.value = pl.id;
}

function onDrop(pl: FlowPlaylist, e: DragEvent) {
    dropId.value = undefined;
    if (pl.filter) return;
    const raw = e.dataTransfer?.getData(MIME);
    if (!raw) return;
    e.preventDefault();
    try {
        const trackIds: number[] = JSON.parse(raw);
        if (!Array.isArray(trackIds) || trackIds.length === 0) return;
        $1t.send('playlistAddTracks', { id: pl.id, trackIds });
        diag('playlistAddTracks', { id: pl.id, tracks: trackIds.length });
        notify({ message: `Added ${trackIds.length} track(s) to "${pl.name}"`, timeout: 1200 });
    } catch { /* foreign drag payload */ }
}
</script>

<style lang='scss' scoped>
.pl-sidebar {
    width: 196px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--ft-line);
    background: var(--ft-panel);
    padding: 8px 6px;
    overflow: hidden;
}
.pl-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 8px 4px;
    .pl-head-actions { display: inline-flex; gap: 2px; }
    .head-label { display: inline-flex; align-items: center; gap: 2px; margin-left: -4px; }
    .head-chevron { width: 13px; height: 13px; }
}
.pl-head-toggle {
    cursor: default;
    &:hover .ft-label { color: var(--ft-text-2); }
}
.pl-list {
    overflow-y: auto;
    min-height: 0;
}
.pl-row {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 6px 0 8px;
    border-radius: 6px;
    cursor: default;
    color: var(--ft-text-2);

    &:hover { background: var(--ft-hover); .pl-menu-btn { opacity: 1; } }
    &.active { background: var(--ft-raised); color: var(--ft-text); }
    &.drop-target { box-shadow: inset 0 0 0 1px var(--ft-focus); }

    .pl-icon { width: 14px; height: 14px; color: var(--ft-text-3); flex-shrink: 0; }
    .pl-name {
        flex: 1;
        min-width: 0;
        font-size: 12.5px;
        font-weight: 500;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .pl-count { font-size: 10.5px; color: var(--ft-text-3); flex-shrink: 0; }
    .pl-rename {
        flex: 1;
        min-width: 0;
        background: var(--ft-bg);
        border: 1px solid var(--ft-focus);
        border-radius: 4px;
        color: var(--ft-text);
        font-size: 12.5px;
        font-weight: 500;
        font-family: inherit;
        padding: 2px 5px;
        outline: none;
    }
    .pl-menu-btn { opacity: 0; padding: 0 3px; height: 20px; flex-shrink: 0; }
    .pl-menu-btn[data-state='open'] { opacity: 1; background: var(--ft-active); }
}
.pl-stage {
    flex-wrap: wrap;
    height: auto;
    min-height: 30px;
    padding-top: 6px;
    padding-bottom: 6px;
    margin-bottom: 4px;
    row-gap: 5px;
    .stage-commit { height: 20px; padding: 0 6px; font-size: 11px; flex-shrink: 0; }
    .stage-bar {
        flex-basis: 100%;
        height: 2px;
        border-radius: 1px;
        background: var(--ft-line);
        overflow: hidden;
    }
    .stage-bar-fill {
        height: 100%;
        background: var(--ft-playing);
        transition: width 300ms ease-out;
    }
}
.pl-empty {
    font-size: 11.5px;
    color: var(--ft-text-3);
    padding: 10px 8px;
    line-height: 1.5;
}
</style>

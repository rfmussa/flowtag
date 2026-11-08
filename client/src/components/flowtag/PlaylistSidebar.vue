<template>
<div class='pl-sidebar'>
    <!-- Collection (everything) -->
    <div
        class='pl-row'
        :class='{ active: store.activePlaylistId.value === undefined && store.activeSourceId.value === undefined }'
        @click='select(undefined)'
    >
        <q-icon name='mdi-music-box-multiple-outline' size='14px' class='pl-icon' />
        <span class='pl-name'>Collection</span>
        <span class='pl-count ft-num'>{{ store.tracks.value.length.toLocaleString() }}</span>
    </div>

    <!-- One scope per source folder -->
    <template v-if='store.folders.value.length > 1'>
        <div class='pl-head'><span class='ft-label'>Sources</span></div>
        <div
            v-for='f in store.folders.value'
            :key='"src" + f.id'
            class='pl-row'
            :class='{ active: store.activeSourceId.value === f.id }'
            :title='f.path'
            @click='store.selectSource(f.id)'
        >
            <q-icon name='mdi-folder-music-outline' size='14px' class='pl-icon' />
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

    <div class='pl-head'>
        <span class='ft-label'>Playlists</span>
        <span class='pl-head-actions'>
            <button
                v-if='filtersActive'
                type='button' class='ft-btn icon-only'
                title='Save the current filter as a smart playlist'
                @click='saveFilterAsPlaylist'
            >
                <q-icon name='mdi-filter-plus-outline' size='14px' />
            </button>
            <button type='button' class='ft-btn icon-only' title='New playlist' @click='createPlaylist'>
                <q-icon name='mdi-plus' size='15px' />
            </button>
        </span>
    </div>

    <div class='pl-list'>
        <div
            v-for='pl in store.playlists.value'
            :key='pl.id'
            class='pl-row'
            :class='{ active: store.activePlaylistId.value === pl.id, "drop-target": dropId === pl.id && !pl.filter }'
            @click='select(pl.id)'
            @dragover='onDragOver(pl, $event)'
            @dragleave='dropId = dropId === pl.id ? undefined : dropId'
            @drop='onDrop(pl, $event)'
        >
            <q-icon :name='pl.filter ? "mdi-filter-outline" : "mdi-playlist-music"' size='14px' class='pl-icon' />
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
            <button type='button' class='ft-btn icon-only pl-menu-btn' aria-label='Playlist menu' @click.stop>
                <q-icon name='mdi-dots-horizontal' size='13px' />
                <q-menu class='flowtag-theme ft-pop' auto-close>
                    <div class='pl-menu'>
                        <button v-if='pl.filter' type='button' class='ft-btn pl-menu-item' @click='rulesFor = pl'>Edit rules</button>
                        <button type='button' class='ft-btn pl-menu-item' @click='startRename("playlist", pl.id)'>Rename</button>
                        <button type='button' class='ft-btn pl-menu-item danger' @click='deletePlaylist(pl)'>Delete</button>
                    </div>
                </q-menu>
            </button>
        </div>
        <div v-if='store.playlists.value.length === 0' class='pl-empty'>
            Drag tracks here, or save a filter as a playlist.
        </div>
    </div>

    <SmartPlaylistEditor :playlist='rulesFor' @close='rulesFor = undefined' />
</div>
</template>

<script lang='ts' setup>
import { computed, ref } from 'vue';
import { Dialog, Notify } from 'quasar';
import { get1t } from '../../scripts/onetagger';
import { getTagStore, FlowPlaylist } from '../../scripts/tagstore';
import SmartPlaylistEditor from './SmartPlaylistEditor.vue';
import { diag } from '../../scripts/diag';

const $1t = get1t();
const store = getTagStore();
const dropId = ref<number | undefined>(undefined);
const rulesFor = ref<FlowPlaylist | undefined>(undefined);
/** Inline rename target (double-click a name, or the menu's Rename) */
const editing = ref<{ kind: 'playlist' | 'source'; id: number } | undefined>(undefined);

const vFocus = { mounted: (el: HTMLInputElement) => { el.focus(); el.select(); } };

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
    || store.untaggedOnly.value);

function select(id?: number) {
    if (id === undefined) store.selectSource(undefined);
    store.selectPlaylist(id);
}

/** Smart playlists count live; manual ones use the server's count */
function playlistCount(pl: FlowPlaylist): number {
    return pl.filter ? store.playlistTrackIds(pl).length : pl.count;
}

function namePrompt(title: string, initial = '', onOk: (name: string) => void) {
    Dialog.create({
        title,
        class: 'flowtag-theme',
        prompt: { model: initial, type: 'text' },
        cancel: true,
    }).onOk((name: string) => {
        if (name?.trim()) onOk(name.trim());
    });
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
        untaggedOnly: store.untaggedOnly.value || undefined,
    };
    namePrompt('Save filter as playlist', '', (name) => {
        $1t.send('playlistCreate', { name, filter: JSON.stringify(saved) });
        diag('playlistCreate', { smart: true });
    });
}

function deletePlaylist(pl: FlowPlaylist) {
    Dialog.create({
        title: 'Delete playlist',
        message: pl.filter
            ? `Delete "${pl.name}"? The saved filter is lost; tracks are not touched.`
            : `Delete "${pl.name}"? Its ${pl.count} track(s) stay in the library.`,
        class: 'flowtag-theme',
        cancel: true,
        ok: { label: 'Delete', color: 'red', flat: true },
    }).onOk(() => {
        $1t.send('playlistDelete', { id: pl.id });
        diag('playlistDelete', { id: pl.id, smart: !!pl.filter });
    });
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
        Notify.create({ message: `Added ${trackIds.length} track(s) to "${pl.name}"`, timeout: 1200 });
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

    .pl-icon { color: var(--ft-text-3); flex-shrink: 0; }
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
}
.pl-menu {
    display: flex;
    flex-direction: column;
    padding: 4px;
    min-width: 120px;
    .pl-menu-item { justify-content: flex-start; width: 100%; }
    .danger { color: var(--ft-danger); }
}
.pl-empty {
    font-size: 11.5px;
    color: var(--ft-text-3);
    padding: 10px 8px;
    line-height: 1.5;
}
</style>

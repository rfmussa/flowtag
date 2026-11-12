import { ref, Ref, computed, watch, WritableComputedRef } from 'vue';
import { notify } from '@/lib/notify';

import { get1t } from './onetagger';
import { diag, diagError } from './diag';
import { QuickFilter, StatusValue, isStatusValue, matchesStatus, passesQuickFilter } from '@/lib/status';

// Types mirroring onetagger-db models (serde camelCase)

export interface FlowTrack {
    id: number;
    path: string;
    folderId: number;
    format: string;
    title: string;
    artist: string;
    durationMs?: number;
    bpm?: number;
    key?: string;
    year?: number;
    /** Unix seconds when the track entered the library */
    addedAt: number;
    missing: boolean;
    needsFlush: boolean;
    tagIds: number[];
    /** Unix seconds when marked done (local only), null/absent = not done */
    doneAt?: number | null;
    /** Member of the stage */
    staged?: boolean;
    /** Stage member flagged to skip at commit */
    skip?: boolean;
}

/** One row of the stage as the server reports it */
export interface StageRow {
    trackId: number;
    skip: boolean;
    ready: boolean;
}

/** View scope: whole library, the stage, or a playlist id */
export type Scope = 'all' | 'stage' | number;

export type SortField = 'path' | 'title' | 'artist' | 'bpm' | 'key' | 'year' | 'added';

/** A playlist: manual (explicit members) or smart (saved filter) */
export interface FlowPlaylist {
    id: number;
    name: string;
    position: number;
    filter?: string | null;
    /** Manual membership count (0 for smart; the client evaluates those) */
    count: number;
}

/** The saved-filter shape a smart playlist stores. All rules AND together;
 *  `tagMode` controls how the include-tags combine among themselves. */
export interface SavedFilter {
    text?: string;
    tagIds?: number[];
    tagMode?: 'all' | 'any';
    excludeTagIds?: number[];
    untaggedOnly?: boolean;
    /** Status rule: `is` must match, `isNot` must not (values per StatusValue) */
    status?: { is?: string; isNot?: string };
    bpmMin?: number;
    bpmMax?: number;
    /** Uppercased key strings as they appear on tracks ('7A', 'AM', ...) */
    keys?: string[];
    yearMin?: number;
    yearMax?: number;
    /** Added to the library within the last N days */
    addedDays?: number;
}

export function parseSavedFilter(raw: string): SavedFilter {
    try {
        const parsed = JSON.parse(raw);
        return (parsed && typeof parsed === 'object') ? parsed : {};
    } catch {
        return {};
    }
}

export interface FlowTag {
    id: number;
    name: string;
    color: string;
    groupId?: number;
    keybind?: any;
    count: number;
}

export interface FlowGroup {
    id: number;
    name: string;
    color: string;
    position: number;
}

export interface FlowFolder {
    id: number;
    path: string;
    /** Display name (defaults to the path's basename; renameable) */
    name: string;
}

export interface ScanState {
    running: boolean;
    done: number;
    total: number;
    result?: { added: number; updated: number; removed: number; failed: number };
}

export interface FlushState {
    pending: number;
    errors: { trackId: number; error: string }[];
}

/** Standard file metadata edited in the track inspector (E) */
export interface TrackMeta {
    title: string;
    artist: string;
    album: string;
    genre: string;
    year?: number | null;
    bpm?: number | null;
    key: string;
    comment: string;
}

/**
 * Client-side state for the FlowTag view. All filtering/sorting/autocomplete runs
 * on this in-memory data; the websocket only carries mutations and their echoes.
 * Updates are immutable (arrays/objects replaced, not mutated) so Vue reactivity
 * stays predictable.
 */
export class TagStore {
    tracks: Ref<FlowTrack[]> = ref([]);
    tags: Ref<FlowTag[]> = ref([]);
    groups: Ref<FlowGroup[]> = ref([]);
    folders: Ref<FlowFolder[]> = ref([]);
    scan: Ref<ScanState> = ref({ running: false, done: 0, total: 0 });
    flush: Ref<FlushState> = ref({ pending: 0, errors: [] });
    loaded: Ref<boolean> = ref(false);

    /** Active track id (loaded in the player; the anchor of the selection) */
    selectedId: Ref<number | undefined> = ref(undefined);
    /** Additional selected track ids (multi-select); tag edits apply to all */
    selectedIds: Ref<number[]> = ref([]);
    /** Incremented to ask the TagInput component to focus itself */
    focusInputSignal: Ref<number> = ref(0);
    /** Incremented to ask the header filter field to focus itself */
    focusFilterSignal: Ref<number> = ref(0);
    /** Incremented to open the track inspector (E) for the selected track */
    inspectorSignal: Ref<number> = ref(0);
    /** Incremented to open the online lookup / Manual Tag (M) for the selected track */
    manualTagSignal: Ref<number> = ref(0);
    /** Latest trackMeta reply (inspector fills its form from this) */
    inspectorMeta: Ref<{ trackId: number; meta: TrackMeta } | undefined> = ref(undefined);
    playlists: Ref<FlowPlaylist[]> = ref([]);
    /** 'all' = whole library, 'stage' = staged tracks, number = playlist id */
    scope: Ref<Scope> = ref('all');
    /** The playlist id of the scope (undefined when scoped to all/stage) */
    activePlaylistId: WritableComputedRef<number | undefined> = computed({
        get: () => (typeof this.scope.value === 'number' ? this.scope.value : undefined),
        set: (id) => { this.scope.value = id === undefined ? 'all' : id; },
    });
    /** Stage summary from the server's stageState broadcast */
    stageReady: Ref<number> = ref(0);
    stageTotal: Ref<number> = ref(0);
    libraryDone: Ref<number> = ref(0);
    /** folder id -> [done, total] */
    folderCounts: Ref<Map<number, [number, number]>> = ref(new Map());
    stageRows: Ref<StageRow[]> = ref([]);
    /** Source-folder scope (mutually exclusive with a playlist scope) */
    activeSourceId: Ref<number | undefined> = ref(undefined);
    /** Manual membership cache: playlist id -> track ids */
    playlistMembers: Ref<Map<number, number[]>> = ref(new Map());
    /** Bumped by the keyboard layer to toggle the playlists sidebar */
    sidebarSignal: Ref<number> = ref(0);
    /** Section markers of the started track (experimental analysis) */
    trackAnalysis: Ref<{ trackId: number; lufs?: number; introMs?: number; dropMs?: number; outroMs?: number } | undefined> = ref(undefined);
    /** Analysis run progress (Settings drives it) */
    analysisRun: Ref<{ running: boolean; done: number; total: number }> = ref({ running: false, done: 0, total: 0 });
    /** Set by shift+Space on a not-yet-started track: seek once markers arrive */
    pendingDropSeek = false;

    /** Player state mirrored from whichever window is driving playback
     *  (nowPlayingChanged broadcast); `at` = wall clock of the snapshot */
    nowPlaying: Ref<{ playing: boolean; title: string; artist: string; positionMs: number; durationMs: number; path: string; at: number } | undefined> = ref(undefined);
    /** Incremented when a trackMetaChanged echo lands (a save completed) */
    metaSavedSignal: Ref<number> = ref(0);
    /** Filter text typed in the header */
    filter: Ref<string> = ref('');
    /** Header quick filter: everything, hide done tracks, or untagged only */
    quickFilter: Ref<QuickFilter> = ref('all');
    /** List ordering */
    sortBy: Ref<SortField> = ref('path');
    sortDesc: Ref<boolean> = ref(false);
    /** Active tag filters: a track must carry ALL of these to be visible */
    filterTagIds: Ref<number[]> = ref([]);

    /** Request initial data from the server */
    load() {
        const $1t = get1t();
        $1t.send('tagFolderList');
        $1t.send('tagPoolLoad');
        $1t.send('tagTracksLoad');
        $1t.send('playlistList');
    }

    /** id -> tag map, rebuilt lazily when the pool changes (hot path at 20k tracks) */
    private tagMap: Map<number, FlowTag> = new Map();
    private tagMapSource: FlowTag[] = [];

    /** Fast lookups for chip rendering */
    tagById(id: number): FlowTag | undefined {
        if (this.tagMapSource !== this.tags.value) {
            this.tagMap = new Map(this.tags.value.map((t) => [t.id, t]));
            this.tagMapSource = this.tags.value;
        }
        return this.tagMap.get(id);
    }

    tagByName(name: string): FlowTag | undefined {
        const n = name.trim().toLowerCase();
        return this.tags.value.find((t) => t.name.toLowerCase() == n);
    }

    selectedTrack(): FlowTrack | undefined {
        return this.tracks.value.find((t) => t.id === this.selectedId.value);
    }

    /** Every selected track id: the anchor plus the multi-selection */
    allSelected(): number[] {
        const anchor = this.selectedId.value;
        const ids = new Set(this.selectedIds.value);
        if (anchor !== undefined) ids.add(anchor);
        return [...ids];
    }

    isSelected(id: number): boolean {
        return id === this.selectedId.value || this.selectedIds.value.includes(id);
    }

    /** Row click: plain = single select, toggle (cmd/ctrl) = add/remove from the
     * selection, range (shift) = select from the anchor through this row */
    clickSelect(track: FlowTrack, opts: { toggle?: boolean; range?: boolean } = {}) {
        if (opts.range && this.selectedId.value !== undefined) {
            const visible = this.visibleTracks();
            const from = visible.findIndex((t) => t.id === this.selectedId.value);
            const to = visible.findIndex((t) => t.id === track.id);
            if (from !== -1 && to !== -1) {
                const [lo, hi] = from < to ? [from, to] : [to, from];
                this.selectedIds.value = visible.slice(lo, hi + 1).map((t) => t.id);
                return;
            }
        }
        if (opts.toggle && this.selectedId.value !== undefined && track.id !== this.selectedId.value) {
            this.selectedIds.value = this.selectedIds.value.includes(track.id)
                ? this.selectedIds.value.filter((id) => id !== track.id)
                : [...this.selectedIds.value, track.id];
            return;
        }
        this.selectedIds.value = [];
        this.selectedId.value = track.id;
    }

    /** Drop the multi-selection, keeping the anchor */
    clearMultiSelect() {
        this.selectedIds.value = [];
    }

    /** Toggle a tag in the AND-filter (crate browsing: show tracks carrying all) */
    toggleTagFilter(tagId: number) {
        this.filterTagIds.value = this.filterTagIds.value.includes(tagId)
            ? this.filterTagIds.value.filter((id) => id !== tagId)
            : [...this.filterTagIds.value, tagId];
    }

    clearTagFilters() {
        this.filterTagIds.value = [];
    }

    /** track id -> lowercased searchable text (title/artist/path/tag names),
     *  rebuilt lazily when the tracks or the tag pool are replaced, so a
     *  filter keystroke costs one `includes` per track instead of repeated
     *  lowercasing and tag lookups over the whole library */
    private searchCache: Map<number, string> = new Map();
    private searchCacheTracks: FlowTrack[] = [];
    private searchCacheTags: FlowTag[] = [];

    private haystack(t: FlowTrack): string {
        if (this.searchCacheTracks !== this.tracks.value || this.searchCacheTags !== this.tags.value) {
            const next = new Map<number, string>();
            for (const track of this.tracks.value) {
                const tagNames = track.tagIds.map((id) => this.tagById(id)?.name ?? '').join('\n');
                // Newline separators keep a query from matching across fields
                next.set(track.id, `${track.title}\n${track.artist}\n${track.path}\n${tagNames}`.toLowerCase());
            }
            this.searchCache = next;
            this.searchCacheTracks = this.tracks.value;
            this.searchCacheTags = this.tags.value;
        }
        return this.searchCache.get(t.id) ?? '';
    }

    /** One filter predicate for live controls and saved (smart) filters alike */
    private passes(t: FlowTrack, text: string, tagFilters: number[], quick: QuickFilter): boolean {
        if (!passesQuickFilter(t, quick)) return false;
        if (tagFilters.length > 0 && !tagFilters.every((id) => t.tagIds.includes(id))) return false;
        return !text || this.haystack(t).includes(text);
    }

    /** Does the track satisfy a smart playlist's saved rules? */
    passesSaved(t: FlowTrack, f: SavedFilter, now: number): boolean {
        if (f.untaggedOnly && t.tagIds.length > 0) return false;
        if (!this.passesStatusRule(t, f)) return false;
        const include = f.tagIds ?? [];
        if (include.length > 0) {
            const hit = (f.tagMode ?? 'all') === 'any'
                ? include.some((id) => t.tagIds.includes(id))
                : include.every((id) => t.tagIds.includes(id));
            if (!hit) return false;
        }
        if ((f.excludeTagIds ?? []).some((id) => t.tagIds.includes(id))) return false;
        if (f.bpmMin !== undefined && !(t.bpm !== undefined && t.bpm >= f.bpmMin)) return false;
        if (f.bpmMax !== undefined && !(t.bpm !== undefined && t.bpm <= f.bpmMax)) return false;
        if (f.keys && f.keys.length > 0
            && !(t.key && f.keys.includes(t.key.trim().toUpperCase()))) return false;
        if (f.yearMin !== undefined && !(t.year !== undefined && t.year >= f.yearMin)) return false;
        if (f.yearMax !== undefined && !(t.year !== undefined && t.year <= f.yearMax)) return false;
        if (f.addedDays !== undefined
            && !(t.addedAt && t.addedAt * 1000 >= now - f.addedDays * 86_400_000)) return false;
        const text = (f.text ?? '').trim().toLowerCase();
        return !text || this.haystack(t).includes(text);
    }

    /** Optional Status rule of a saved filter; unknown values are ignored */
    private passesStatusRule(t: FlowTrack, f: SavedFilter): boolean {
        const { is, isNot } = f.status ?? {};
        if (isStatusValue(is) && !matchesStatus(t, { status: is })) return false;
        if (isStatusValue(isNot) && !matchesStatus(t, { status: isNot, negate: true })) return false;
        return true;
    }

    /** The list as currently shown: playlist scope + filter + untagged + sort.
     *  A manual playlist scopes by membership, a smart one by its rules; the
     *  live controls filter within that scope. */
    visibleTracks(): FlowTrack[] {
        const f = this.filter.value.trim().toLowerCase();
        const tagFilters = this.filterTagIds.value;
        const stageOnly = this.scope.value === 'stage';
        const active = this.activePlaylist();
        const members = active && !active.filter
            ? new Set(this.playlistMembers.value.get(active.id) ?? [])
            : undefined;
        const saved = active?.filter ? parseSavedFilter(active.filter) : undefined;
        const now = Date.now();
        const sourceId = this.activeSourceId.value;
        const filtered = this.tracks.value.filter((t) => {
            if (sourceId !== undefined && t.folderId !== sourceId) return false;
            if (stageOnly && t.staged !== true) return false;
            if (members && !members.has(t.id)) return false;
            if (saved && !this.passesSaved(t, saved, now)) return false;
            return this.passes(t, f, tagFilters, this.quickFilter.value);
        });
        return this.sortTracks(filtered);
    }

    activePlaylist(): FlowPlaylist | undefined {
        const id = this.activePlaylistId.value;
        return id === undefined ? undefined : this.playlists.value.find((p) => p.id === id);
    }

    /** Switch the view scope. Selecting a smart playlist loads its saved
     *  filter into the live controls (a saved search, visibly). */
    /** Scope the view to one source folder (or back to the whole collection) */
    selectSource(id?: number) {
        this.activeSourceId.value = id;
        this.activePlaylistId.value = undefined;
        if (id !== undefined) diag('sourceSelect', { id });
    }

    /** Scope the view to the stage */
    selectStage() {
        this.activeSourceId.value = undefined;
        this.scope.value = 'stage';
        diag('stageSelect');
    }

    selectPlaylist(id?: number) {
        this.activePlaylistId.value = id;
        this.activeSourceId.value = undefined;
        const pl = id === undefined ? undefined : this.playlists.value.find((p) => p.id === id);
        if (!pl) {
            if (id !== undefined) this.activePlaylistId.value = undefined;
            return;
        }
        if (!pl.filter && !this.playlistMembers.value.has(pl.id)) {
            get1t().send('playlistTracksLoad', { id: pl.id });
        }
        diag('playlistSelect', { id, smart: !!pl.filter });
    }

    /** Every track a playlist resolves to right now (manual members, or the
     *  smart filter evaluated against the library) */
    playlistTrackIds(pl: FlowPlaylist): number[] {
        if (!pl.filter) return this.playlistMembers.value.get(pl.id) ?? [];
        const f = parseSavedFilter(pl.filter);
        const now = Date.now();
        return this.tracks.value.filter((t) => this.passesSaved(t, f, now)).map((t) => t.id);
    }

    private sortTracks(tracks: FlowTrack[]): FlowTrack[] {
        const field = this.sortBy.value;
        if (field === 'path' && !this.sortDesc.value) return tracks; // server order
        const dir = this.sortDesc.value ? -1 : 1;
        // Tracks without the sort value always sink to the bottom, in either direction
        const nums = (a: number | undefined, b: number | undefined): number => {
            if (a === undefined && b === undefined) return 0;
            if (a === undefined) return dir;
            if (b === undefined) return -dir;
            return a - b;
        };
        const cmp = (a: FlowTrack, b: FlowTrack): number => {
            switch (field) {
                case 'title': return a.title.localeCompare(b.title);
                case 'artist': return a.artist.localeCompare(b.artist) || a.title.localeCompare(b.title);
                case 'bpm': return nums(a.bpm, b.bpm);
                case 'year': return nums(a.year, b.year);
                case 'added': return a.addedAt - b.addedAt;
                case 'key': {
                    if (!a.key && !b.key) return 0;
                    if (!a.key) return dir;
                    if (!b.key) return -dir;
                    return a.key.localeCompare(b.key, undefined, { numeric: true });
                }
                default: return a.path.localeCompare(b.path);
            }
        };
        return [...tracks].sort((a, b) => dir * cmp(a, b));
    }

    // ---- Mutations (optimistic, reconciled by server echoes) ----

    /** Undo history: each entry is the information needed to invert one edit */
    private undoStack: { op: 'add' | 'remove'; trackIds: number[]; tagName: string }[] = [];
    private static readonly UNDO_LIMIT = 50;

    private recordUndo(entry: { op: 'add' | 'remove'; trackIds: number[]; tagName: string }) {
        this.undoStack.push(entry);
        if (this.undoStack.length > TagStore.UNDO_LIMIT) this.undoStack.shift();
    }

    /** Invert the most recent tag edit. Returns a description, or undefined. */
    undo(): string | undefined {
        const entry = this.undoStack.pop();
        if (!entry) return undefined;
        diag('undo', { op: entry.op, name: entry.tagName, tracks: entry.trackIds.length });
        if (entry.op === 'add') {
            const tag = this.tagByName(entry.tagName);
            if (tag) this.performRemove(entry.trackIds.filter((id) =>
                this.tracks.value.find((t) => t.id === id)?.tagIds.includes(tag.id)), tag.id);
            return `Removed "${entry.tagName}" from ${entry.trackIds.length} track(s)`;
        }
        this.performAdd(entry.trackIds, entry.tagName);
        return `Re-added "${entry.tagName}" to ${entry.trackIds.length} track(s)`;
    }

    /** Add a tag by name to a track — or to the whole multi-selection when the
     * track is part of it. Known tags apply instantly; new ones on echo. */
    addTag(trackId: number, name: string) {
        const trimmed = name.trim();
        if (!trimmed) return;
        const targets = this.bulkTargets(trackId);
        const known = this.tagByName(trimmed);
        // Only count tracks the tag actually lands on (no-ops skew undo)
        const effective = known
            ? targets.filter((id) => !this.tracks.value.find((t) => t.id === id)?.tagIds.includes(known.id))
            : targets;
        if (known && effective.length === 0) return; // no-op everywhere
        diag('tagAdd', { tracks: targets.length, name: trimmed });
        this.performAdd(targets, trimmed);
        this.recordUndo({ op: 'add', trackIds: effective, tagName: trimmed });
    }

    /** Remove a tag from a track — or from the whole multi-selection (optimistic) */
    removeTag(trackId: number, tagId: number) {
        const targets = this.bulkTargets(trackId).filter((id) =>
            this.tracks.value.find((t) => t.id === id)?.tagIds.includes(tagId));
        if (targets.length === 0) return;
        const tagName = this.tagById(tagId)?.name;
        diag('tagRemove', { tracks: targets.length, name: tagName });
        this.performRemove(targets, tagId);
        if (tagName) this.recordUndo({ op: 'remove', trackIds: targets, tagName });
    }

    /** Optimistic add + send, no undo bookkeeping (used by addTag and undo) */
    private performAdd(targets: number[], name: string) {
        if (targets.length === 0) return;
        const known = this.tagByName(name);
        if (known) {
            for (const id of targets) {
                const track = this.tracks.value.find((t) => t.id === id);
                if (track && !track.tagIds.includes(known.id)) {
                    this.replaceTrackTags(id, (ids) => [...ids, known.id]);
                    this.bumpCount(known.id, 1);
                }
            }
        }
        if (targets.length > 1) {
            get1t().send('tracksAddTag', { trackIds: targets, name });
        } else {
            get1t().send('trackAddTag', { trackId: targets[0], name });
        }
    }

    /** Optimistic remove + send, no undo bookkeeping (used by removeTag and undo) */
    private performRemove(targets: number[], tagId: number) {
        if (targets.length === 0) return;
        for (const id of targets) {
            this.replaceTrackTags(id, (ids) => ids.filter((i) => i !== tagId));
            this.bumpCount(tagId, -1);
        }
        if (targets.length > 1) {
            get1t().send('tracksRemoveTag', { trackIds: targets, tagId });
        } else {
            get1t().send('trackRemoveTag', { trackId: targets[0], tagId });
        }
    }

    /** The ids a tag edit on `trackId` applies to: the multi-selection when the
     * track belongs to it, otherwise just that track */
    private bulkTargets(trackId: number): number[] {
        const all = this.allSelected();
        return all.length > 1 && all.includes(trackId) ? all : [trackId];
    }

    /** Toggle a tag (used by per-tag hotkeys and palette clicks). With a
     * multi-selection the anchor decides: has it -> remove everywhere,
     * lacks it -> add everywhere. */
    toggleTag(trackId: number, tag: FlowTag) {
        const track = this.tracks.value.find((t) => t.id === trackId);
        if (!track) return;
        if (track.tagIds.includes(tag.id)) {
            this.removeTag(trackId, tag.id);
        } else {
            this.addTag(trackId, tag.name);
        }
    }

    /** Autocomplete: ranked prefix-then-substring match over the pool */
    suggest(query: string, limit = 8): FlowTag[] {
        const q = query.trim().toLowerCase();
        if (!q) {
            // Empty query: most used tags first
            return [...this.tags.value].sort((a, b) => b.count - a.count).slice(0, limit);
        }
        const prefix: FlowTag[] = [];
        const substr: FlowTag[] = [];
        for (const tag of this.tags.value) {
            const n = tag.name.toLowerCase();
            if (n.startsWith(q)) prefix.push(tag);
            else if (n.includes(q)) substr.push(tag);
        }
        const rank = (arr: FlowTag[]) => arr.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
        return [...rank(prefix), ...rank(substr)].slice(0, limit);
    }

    // ---- Selection / navigation ----

    /** Last known position of the selection within the visible list. Keeps the
     * user's place when the selected track drops out of the view (e.g. tagging
     * a track while "Untagged only" is on). */
    private lastVisibleIndex = 0;

    /** The selection's index in `visible`, falling back to its last known place */
    private anchorIndex(visible: FlowTrack[]): number {
        const idx = visible.findIndex((t) => t.id === this.selectedId.value);
        if (idx !== -1) {
            this.lastVisibleIndex = idx;
            return idx;
        }
        return Math.min(this.lastVisibleIndex, Math.max(visible.length - 1, 0));
    }

    /** Select by offset within the visible list; `extend` grows the multi-selection */
    selectOffset(offset: number, extend = false) {
        const visible = this.visibleTracks();
        if (visible.length === 0) return;
        const idx = this.anchorIndex(visible);
        const stillVisible = visible[idx]?.id === this.selectedId.value;
        // When the current track vanished from the view, the anchor position
        // already points at the "next" row, so a +1 step stays in place
        const step = !stillVisible && offset > 0 ? offset - 1 : offset;
        const next = Math.min(Math.max(idx + step, 0), visible.length - 1);
        if (extend && this.selectedId.value !== undefined) {
            // Keep the old anchor selected while the anchor walks on
            if (!this.selectedIds.value.includes(this.selectedId.value)) {
                this.selectedIds.value = [...this.selectedIds.value, this.selectedId.value];
            }
        } else {
            this.selectedIds.value = [];
        }
        this.selectedId.value = visible[next].id;
        this.lastVisibleIndex = next;
    }

    /** Jump to the next untagged track after the current one (wraps around) */
    selectNextUntagged() {
        const visible = this.visibleTracks();
        if (visible.length === 0) return;
        const stillVisible = visible.some((t) => t.id === this.selectedId.value);
        const start = this.anchorIndex(visible);
        // Anchor already points past a vanished track, so include it in the walk
        const from = stillVisible ? start : start - 1;
        for (let i = 1; i <= visible.length; i++) {
            const t = visible[(from + i + visible.length) % visible.length];
            if (t.tagIds.length === 0) {
                this.selectedIds.value = [];
                this.selectedId.value = t.id;
                this.lastVisibleIndex = visible.indexOf(t);
                return;
            }
        }
    }

    // ---- Staging ----

    /** A stageCommitted arrived and the next stageState has not yet been seen */
    private commitLanded = false;

    /** Bumped on every server error reply */
    errorSeq: Ref<number> = ref(0);

    /** The newest commit the server confirmed */
    lastCommit: Ref<{ id: number; trackCount: number } | undefined> = ref(undefined);

    /** Commit with an empty message; resolves once the server confirms, replies
     *  with an error, or after `timeoutMs`. Rejects only if the send itself throws. */
    commitStageAndWait(trackIds: number[], timeoutMs: number): Promise<void> {
        const lastId = this.lastCommit.value?.id;
        const lastError = this.errorSeq.value;
        return new Promise((resolve, reject) => {
            const done = () => { stopCommit(); stopError(); clearTimeout(timer); resolve(); };
            const stopCommit = watch(this.lastCommit, (c) => { if (c && c.id !== lastId) done(); });
            const stopError = watch(this.errorSeq, (n) => { if (n !== lastError) done(); });
            const timer = setTimeout(done, timeoutMs);
            try {
                this.commitStage(trackIds, '');
            } catch (e) {
                stopCommit(); stopError(); clearTimeout(timer);
                reject(e);
            }
        });
    }

    /** Commit the given staged tracks with a message */
    commitStage(trackIds: number[], message: string) {
        get1t().send('stageCommit', { trackIds, message });
    }

    /** Rows only learn staged/skip/doneAt from a row load: refetch after our own stage mutations */
    private refreshRows() {
        get1t().send('tagTracksLoad');
    }

    /** First `limit` ids of `ids` that are neither done nor staged, in the given order */
    notDoneUnstagedIds(ids: number[], limit = 200): number[] {
        const byId = new Map(this.tracks.value.map((t) => [t.id, t]));
        const out: number[] = [];
        for (const id of ids) {
            const t = byId.get(id);
            if (!t || (t.doneAt !== undefined && t.doneAt !== null) || t.staged) continue;
            out.push(id);
            if (out.length >= limit) break;
        }
        return out;
    }

    stageTrackIds(trackIds: number[]) {
        if (trackIds.length === 0) return;
        const $1t = get1t();
        $1t.send('stageTracks', { trackIds });
        this.refreshRows();
    }

    unstageTrackIds(trackIds: number[]) {
        if (trackIds.length === 0) return;
        get1t().send('unstageTracks', { trackIds });
        this.refreshRows();
    }

    /** Not-done, not-staged track ids of a source folder (library order, capped) */
    folderNotDoneUnstagedIds(folderId: number, limit = 200): number[] {
        return this.notDoneUnstagedIds(
            this.tracks.value.filter((t) => t.folderId === folderId).map((t) => t.id), limit);
    }

    /** Stage the first 50 not-done, not-staged tracks in the current view order */
    stageNext50() {
        this.stageTrackIds(this.notDoneUnstagedIds(this.visibleTracks().map((t) => t.id), 50));
    }

    /** Every selected track staged -> unstage them; otherwise stage the ones that are not */
    toggleStageSelection() {
        const byId = new Map(this.tracks.value.map((t) => [t.id, t]));
        const selected = this.allSelected().map((id) => byId.get(id)).filter((t): t is FlowTrack => !!t);
        if (selected.length === 0) return;
        if (selected.every((t) => t.staged)) {
            this.unstageTrackIds(selected.map((t) => t.id));
        } else {
            this.stageTrackIds(selected.filter((t) => !t.staged).map((t) => t.id));
        }
    }

    /** X: toggle skip on the current track (server-side toggle) */
    stageSkipCurrent() {
        const id = this.selectedId.value;
        if (id === undefined) return;
        get1t().send('stageSkip', { trackId: id });
        this.refreshRows();
    }

    /** Stage scope D: jump to the next staged row that is not ready (wraps) */
    selectNextStagedNotReady() {
        const notReady = new Set(this.stageRows.value.filter((r) => !r.ready).map((r) => r.trackId));
        const visible = this.visibleTracks();
        if (visible.length === 0 || notReady.size === 0) return;
        const stillVisible = visible.some((t) => t.id === this.selectedId.value);
        const start = this.anchorIndex(visible);
        const from = stillVisible ? start : start - 1;
        for (let i = 1; i <= visible.length; i++) {
            const t = visible[(from + i + visible.length) % visible.length];
            if (notReady.has(t.id)) {
                this.selectedIds.value = [];
                this.selectedId.value = t.id;
                this.lastVisibleIndex = visible.indexOf(t);
                return;
            }
        }
    }

    /** The removed track's neighbour inherits the cursor (wraps, never null while rows remain) */
    selectNeighbourAfterRemoval(trackId: number) {
        if (this.selectedId.value !== trackId) return;
        const visible = this.visibleTracks();
        if (visible.length === 0 || visible.some((t) => t.id === trackId)) return;
        // The row after the vanished one now sits at its old index
        const idx = this.lastVisibleIndex % visible.length;
        this.selectedIds.value = [];
        this.selectedId.value = visible[idx].id;
        this.lastVisibleIndex = idx;
    }

    // ---- Incoming websocket events ----

    /** Handle a server message. Returns true when consumed. */
    handleMessage(json: any): boolean {
        switch (json.action) {
            case 'tagFolders':
                this.folders.value = json.folders ?? [];
                return true;
            case 'tagTracks': {
                // Remember the cursor's place so a vanished row hands it to its neighbour
                const current = this.selectedId.value;
                if (current !== undefined) this.anchorIndex(this.visibleTracks());
                this.tracks.value = json.tracks ?? [];
                if (current !== undefined) this.selectNeighbourAfterRemoval(current);
                this.loaded.value = true;
                diag('libraryLoaded', { tracks: this.tracks.value.length });
                return true;
            }
            case 'stageCommitted':
                this.commitLanded = true;
                if (json.commit) this.lastCommit.value = { id: json.commit.id, trackCount: json.commit.trackCount };
                this.refreshRows();
                return false;
            case 'stageState':
                this.stageReady.value = json.ready ?? 0;
                this.stageTotal.value = json.total ?? 0;
                this.libraryDone.value = json.libraryDone ?? 0;
                this.folderCounts.value = new Map(
                    ((json.folderCounts ?? []) as [number, number, number][])
                        .map(([id, done, total]): [number, [number, number]] => [id, [done, total]]));
                this.stageRows.value = json.stage ?? [];
                // A commit that empties the stage leaves nothing to show in its scope
                if (this.commitLanded && this.stageTotal.value === 0 && this.scope.value === 'stage') this.scope.value = 'all';
                this.commitLanded = false;
                return true;
            case 'tagPool':
                this.tags.value = json.tags ?? [];
                this.groups.value = json.groups ?? [];
                return true;
            case 'tagScanProgress':
                this.scan.value = { ...this.scan.value, running: true, done: json.done, total: json.total };
                return true;
            case 'tagScanDone':
                this.scan.value = { running: false, done: 0, total: 0, result: json.result };
                diag('scanDone', json.result ?? {});
                return true;
            case 'tagCreated':
            case 'tagUpdated':
                this.upsertPoolTag(json.tag);
                return true;
            case 'tagDeleted':
                this.tags.value = this.tags.value.filter((t) => t.id !== json.id);
                this.tracks.value = this.tracks.value.map((t) =>
                    t.tagIds.includes(json.id) ? { ...t, tagIds: t.tagIds.filter((i) => i !== json.id) } : t);
                return true;
            case 'trackTagsChanged':
                // Authoritative echo: replace local (optimistic) state
                if (json.newTag) this.upsertPoolTag(json.newTag);
                this.tracks.value = this.tracks.value.map((t) =>
                    t.id === json.trackId ? { ...t, tagIds: json.tagIds, needsFlush: true } : t);
                this.refreshCounts();
                return true;
            case 'tracksTagsChanged': {
                // Bulk echo: apply the op to every listed track
                if (json.newTag) this.upsertPoolTag(json.newTag);
                const ids = new Set<number>(json.trackIds ?? []);
                this.tracks.value = this.tracks.value.map((t) => {
                    if (!ids.has(t.id)) return t;
                    const has = t.tagIds.includes(json.tagId);
                    if (json.op === 'add' && !has) return { ...t, tagIds: [...t.tagIds, json.tagId], needsFlush: true };
                    if (json.op === 'remove' && has) return { ...t, tagIds: t.tagIds.filter((i) => i !== json.tagId), needsFlush: true };
                    // No-op for this track: the server didn't dirty it either
                    return t;
                });
                this.refreshCounts();
                return true;
            }
            case 'flushStatus':
                this.flush.value = { pending: json.pending ?? 0, errors: json.errors ?? [] };
                return true;
            case 'trackMeta':
                this.inspectorMeta.value = { trackId: json.trackId, meta: json.meta };
                return true;
            case 'trackMetaChanged': {
                // Mirror the list-view columns; an emptied title falls back to
                // the filename stem, same as the server-side DB row
                const m: TrackMeta = json.meta ?? {};
                this.tracks.value = this.tracks.value.map((t) => {
                    if (t.id !== json.trackId) return t;
                    const stem = t.path.split('/').pop()?.replace(/\.[^.]+$/, '') ?? t.path;
                    return {
                        ...t,
                        title: (m.title ?? '').trim() || stem,
                        artist: (m.artist ?? '').trim(),
                        bpm: m.bpm ?? undefined,
                        key: (m.key ?? '').trim() || undefined,
                        year: m.year ?? undefined,
                    };
                });
                this.metaSavedSignal.value++;
                return true;
            }
            case 'playerStarted': {
                const player = get1t().player.value;
                player.duration = json.duration || 1;
                player.position = 0;
                player.gainDb = json.gainDb ?? 0;
                return true;
            }
            case 'trackAnalysis': {
                this.trackAnalysis.value = {
                    trackId: json.trackId,
                    lufs: json.lufs ?? undefined,
                    introMs: json.introMs ?? undefined,
                    dropMs: json.dropMs ?? undefined,
                    outroMs: json.outroMs ?? undefined,
                };
                if (this.pendingDropSeek) {
                    this.pendingDropSeek = false;
                    const player = get1t().player.value;
                    if (json.dropMs && player.trackId === json.trackId) {
                        player.seek(json.dropMs);
                        player.play();
                    }
                }
                return true;
            }
            case 'analysisProgress':
                this.analysisRun.value = { running: true, done: json.done ?? 0, total: json.total ?? 0 };
                return true;
            case 'analysisDone': {
                this.analysisRun.value = { running: false, done: 0, total: 0 };
                diag('analysisDone', { analyzed: json.analyzed, failed: json.failed });
                notify({ message: `Analysis done: ${json.analyzed} track(s)` + (json.failed ? `, ${json.failed} failed` : ''), timeout: 2500 });
                return true;
            }
            case 'trackWaveform': {
                // Ignore waveforms of tracks the user already moved away from
                if (json.trackId !== this.selectedId.value) return true;
                const player = get1t().player.value;
                if (Array.isArray(json.bars) && json.bars.length > 0) {
                    player.waveform = json.bars;
                }
                return true;
            }
            case 'playlists': {
                this.playlists.value = json.playlists ?? [];
                // The active playlist may have been deleted in another window
                if (this.activePlaylistId.value !== undefined
                    && !this.playlists.value.some((p) => p.id === this.activePlaylistId.value)) {
                    this.activePlaylistId.value = undefined;
                }
                return true;
            }
            case 'playlistTracks': {
                const next = new Map(this.playlistMembers.value);
                next.set(json.id, json.trackIds ?? []);
                this.playlistMembers.value = next;
                return true;
            }
            case 'playlistCreated':
                // Only the creator receives this: jump into the new playlist
                diag('playlistCreated', { id: json.id });
                return true;
            case 'nowPlayingChanged':
                this.nowPlaying.value = {
                    playing: !!json.playing,
                    title: json.title ?? '',
                    artist: json.artist ?? '',
                    positionMs: json.positionMs ?? 0,
                    durationMs: json.durationMs ?? 0,
                    path: json.path ?? '',
                    at: Date.now(),
                };
                return true;
            // OS media keys (play/pause/next from Control Center, AirPods, F8...)
            case 'mediaKey': {
                // Only the main window drives the (single, server-side) player;
                // the miniplayer mirrors state and must not double-execute
                if (window.location.hash.startsWith('#/mini')) return true;
                diag('mediaKey', { key: json.key });
                const player = get1t().player.value;
                switch (json.key) {
                    case 'playPause': player.playing ? player.pause() : player.play(); break;
                    case 'play': if (!player.playing) player.play(); break;
                    case 'pause': if (player.playing) player.pause(); break;
                    // Media next/prev means "play the neighbor", not just select it
                    case 'next': player.wasPlaying = true; this.selectOffset(1, false); break;
                    case 'prev': player.wasPlaying = true; this.selectOffset(-1, false); break;
                    case 'seek':
                        if (typeof json.positionMs === 'number') player.seek(json.positionMs);
                        break;
                }
                return true;
            }
            case 'trackFlushed':
                if (json.ok) {
                    this.tracks.value = this.tracks.value.map((t) =>
                        t.id === json.trackId ? { ...t, needsFlush: false } : t);
                } else if (json.error) {
                    diagError('flushError', { trackId: json.trackId, error: String(json.error).slice(0, 300) });
                    // Surface write failures immediately (not only after a reload)
                    const others = this.flush.value.errors.filter((e) => e.trackId !== json.trackId);
                    this.flush.value = {
                        ...this.flush.value,
                        errors: [...others, { trackId: json.trackId, error: json.error }],
                    };
                }
                return true;
            case 'rekordboxExportProgress':
                return true;
            case 'error':
                this.errorSeq.value++;
                // A failed tag action leaves optimistic state wrong: resync from
                // the server (cheap), but let the app still show the error toast
                if (this.loaded.value && window.location.hash.startsWith('#/quicktag')) {
                    const $1t = get1t();
                    $1t.send('tagTracksLoad');
                    $1t.send('tagPoolLoad');
                }
                return false;
            case 'rekordboxExportDone': {
                const r = json.result ?? {};
                notify({
                    message: `Export done: ${r.written ?? 0} written, ${r.unchanged ?? 0} unchanged` +
                        (r.failed ? `, ${r.failed} FAILED` : ''),
                    color: r.failed ? 'negative' : 'positive',
                    timeout: 4000,
                });
                return true;
            }
        }
        return false;
    }

    // ---- Internals ----

    private replaceTrackTags(trackId: number, fn: (ids: number[]) => number[]) {
        this.tracks.value = this.tracks.value.map((t) =>
            t.id === trackId ? { ...t, tagIds: fn(t.tagIds), needsFlush: true } : t);
    }

    private upsertPoolTag(tag: FlowTag) {
        const existing = this.tags.value.findIndex((t) => t.id === tag.id);
        if (existing === -1) {
            this.tags.value = [...this.tags.value, tag];
        } else {
            this.tags.value = this.tags.value.map((t) => (t.id === tag.id ? tag : t));
        }
    }

    private bumpCount(tagId: number, delta: number) {
        this.tags.value = this.tags.value.map((t) =>
            t.id === tagId ? { ...t, count: Math.max(t.count + delta, 0) } : t);
    }

    /** Recompute tag usage counts from track state (cheap: one pass) */
    private refreshCounts() {
        const counts = new Map<number, number>();
        for (const track of this.tracks.value) {
            for (const id of track.tagIds) {
                counts.set(id, (counts.get(id) ?? 0) + 1);
            }
        }
        this.tags.value = this.tags.value.map((t) => {
            const c = counts.get(t.id) ?? 0;
            return c === t.count ? t : { ...t, count: c };
        });
    }
}

let store: TagStore | undefined;

/** FlowTag state singleton (mirrors the get1t() pattern) */
export function getTagStore(): TagStore {
    if (!store) store = new TagStore();
    return store;
}

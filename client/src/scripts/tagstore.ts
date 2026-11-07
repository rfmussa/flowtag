import { ref, Ref } from 'vue';
import { Notify } from 'quasar';
import { get1t } from './onetagger';

// Types mirroring onetagger-db models (serde camelCase)

export interface FlowTrack {
    id: number;
    path: string;
    format: string;
    title: string;
    artist: string;
    durationMs?: number;
    bpm?: number;
    key?: string;
    year?: number;
    missing: boolean;
    needsFlush: boolean;
    tagIds: number[];
}

export type SortField = 'path' | 'title' | 'artist' | 'bpm' | 'key' | 'year';

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
    /** Filter text typed in the header */
    filter: Ref<string> = ref('');
    /** Show only untagged tracks */
    untaggedOnly: Ref<boolean> = ref(false);
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

    /** The list as currently shown: filter + untagged toggle + sort applied */
    visibleTracks(): FlowTrack[] {
        const f = this.filter.value.trim().toLowerCase();
        const tagFilters = this.filterTagIds.value;
        const filtered = this.tracks.value.filter((t) => {
            if (this.untaggedOnly.value && t.tagIds.length > 0) return false;
            if (tagFilters.length > 0 && !tagFilters.every((id) => t.tagIds.includes(id))) return false;
            if (!f) return true;
            if (t.title.toLowerCase().includes(f)) return true;
            if (t.artist.toLowerCase().includes(f)) return true;
            if (t.path.toLowerCase().includes(f)) return true;
            // Match tag names too
            return t.tagIds.some((id) => this.tagById(id)?.name.toLowerCase().includes(f));
        });
        return this.sortTracks(filtered);
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
        this.performAdd(targets, trimmed);
        this.recordUndo({ op: 'add', trackIds: effective, tagName: trimmed });
    }

    /** Remove a tag from a track — or from the whole multi-selection (optimistic) */
    removeTag(trackId: number, tagId: number) {
        const targets = this.bulkTargets(trackId).filter((id) =>
            this.tracks.value.find((t) => t.id === id)?.tagIds.includes(tagId));
        if (targets.length === 0) return;
        const tagName = this.tagById(tagId)?.name;
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

    // ---- Incoming websocket events ----

    /** Handle a server message. Returns true when consumed. */
    handleMessage(json: any): boolean {
        switch (json.action) {
            case 'tagFolders':
                this.folders.value = json.folders ?? [];
                return true;
            case 'tagTracks':
                this.tracks.value = json.tracks ?? [];
                this.loaded.value = true;
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
            case 'playerStarted': {
                const player = get1t().player.value;
                player.duration = json.duration || 1;
                player.position = 0;
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
            case 'trackFlushed':
                if (json.ok) {
                    this.tracks.value = this.tracks.value.map((t) =>
                        t.id === json.trackId ? { ...t, needsFlush: false } : t);
                } else if (json.error) {
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
                Notify.create({
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

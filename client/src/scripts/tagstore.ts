import { ref, Ref } from 'vue';
import { get1t } from './onetagger';

// Types mirroring onetagger-db models (serde camelCase)

export interface FlowTrack {
    id: number;
    path: string;
    format: string;
    title: string;
    artist: string;
    durationMs?: number;
    missing: boolean;
    needsFlush: boolean;
    tagIds: number[];
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

    /** Selected track id (single selection; the list derives the index) */
    selectedId: Ref<number | undefined> = ref(undefined);
    /** Incremented to ask the TagInput component to focus itself */
    focusInputSignal: Ref<number> = ref(0);
    /** Filter text typed in the header */
    filter: Ref<string> = ref('');
    /** Show only untagged tracks */
    untaggedOnly: Ref<boolean> = ref(false);

    /** Request initial data from the server */
    load() {
        const $1t = get1t();
        $1t.send('tagFolderList');
        $1t.send('tagPoolLoad');
        $1t.send('tagTracksLoad');
    }

    /** Fast lookups for chip rendering */
    tagById(id: number): FlowTag | undefined {
        return this.tags.value.find((t) => t.id == id);
    }

    tagByName(name: string): FlowTag | undefined {
        const n = name.trim().toLowerCase();
        return this.tags.value.find((t) => t.name.toLowerCase() == n);
    }

    selectedTrack(): FlowTrack | undefined {
        return this.tracks.value.find((t) => t.id === this.selectedId.value);
    }

    /** The list as currently shown: filter + untagged toggle applied */
    visibleTracks(): FlowTrack[] {
        const f = this.filter.value.trim().toLowerCase();
        return this.tracks.value.filter((t) => {
            if (this.untaggedOnly.value && t.tagIds.length > 0) return false;
            if (!f) return true;
            if (t.title.toLowerCase().includes(f)) return true;
            if (t.artist.toLowerCase().includes(f)) return true;
            if (t.path.toLowerCase().includes(f)) return true;
            // Match tag names too
            return t.tagIds.some((id) => this.tagById(id)?.name.toLowerCase().includes(f));
        });
    }

    // ---- Mutations (optimistic, reconciled by server echoes) ----

    /** Add a tag by name to a track. Known tags apply instantly; new ones on echo. */
    addTag(trackId: number, name: string) {
        const trimmed = name.trim();
        if (!trimmed) return;
        const known = this.tagByName(trimmed);
        if (known) {
            this.replaceTrackTags(trackId, (ids) =>
                ids.includes(known.id) ? ids : [...ids, known.id]);
            this.bumpCount(known.id, 1);
        }
        get1t().send('trackAddTag', { trackId, name: trimmed });
    }

    /** Remove a tag from a track (optimistic) */
    removeTag(trackId: number, tagId: number) {
        this.replaceTrackTags(trackId, (ids) => ids.filter((i) => i !== tagId));
        this.bumpCount(tagId, -1);
        get1t().send('trackRemoveTag', { trackId, tagId });
    }

    /** Toggle a tag (used by per-tag hotkeys and palette clicks) */
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

    /** Select by offset within the visible list */
    selectOffset(offset: number) {
        const visible = this.visibleTracks();
        if (visible.length === 0) return;
        const idx = visible.findIndex((t) => t.id === this.selectedId.value);
        const next = idx === -1 ? 0 : Math.min(Math.max(idx + offset, 0), visible.length - 1);
        this.selectedId.value = visible[next].id;
    }

    /** Jump to the next untagged track after the current one (wraps around) */
    selectNextUntagged() {
        const visible = this.visibleTracks();
        if (visible.length === 0) return;
        const start = Math.max(visible.findIndex((t) => t.id === this.selectedId.value), 0);
        for (let i = 1; i <= visible.length; i++) {
            const t = visible[(start + i) % visible.length];
            if (t.tagIds.length === 0) {
                this.selectedId.value = t.id;
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
            case 'flushStatus':
                this.flush.value = { pending: json.pending ?? 0, errors: json.errors ?? [] };
                return true;
            case 'trackFlushed':
                if (json.ok) {
                    this.tracks.value = this.tracks.value.map((t) =>
                        t.id === json.trackId ? { ...t, needsFlush: false } : t);
                }
                return true;
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

<template>
<q-dialog :model-value='playlist !== undefined' @update:model-value='(v: boolean) => { if (!v) emit("close"); }'>
    <q-card v-if='playlist' flat class='spe-card flowtag-theme'>
        <div class='ft-title'>{{ playlist.name }} — rules</div>
        <div class='spe-sub'>All rules combine; a track must satisfy every row.</div>

        <div class='spe-body'>
            <div class='spe-row'>
                <span class='spe-label'>Text contains</span>
                <div class='ft-field spe-wide'><input v-model='draft.textInput' placeholder='title, artist, path or tag' /></div>
            </div>

            <div class='spe-row'>
                <span class='spe-label'>Has tags</span>
                <q-select
                    v-model='draft.tagIds' :options='tagOptions' multiple dense options-dense
                    borderless emit-value map-options use-chips class='spe-select spe-wide'
                    popup-content-class='flowtag-theme ft-pop'
                />
                <div class='seg' role='group' aria-label='Tag matching'>
                    <button type='button' class='seg-btn' :class='{ on: draft.tagMode === "all" }' @click='draft.tagMode = "all"'>all</button>
                    <button type='button' class='seg-btn' :class='{ on: draft.tagMode === "any" }' @click='draft.tagMode = "any"'>any</button>
                </div>
            </div>

            <div class='spe-row'>
                <span class='spe-label'>Not tags</span>
                <q-select
                    v-model='draft.excludeTagIds' :options='tagOptions' multiple dense options-dense
                    borderless emit-value map-options use-chips class='spe-select spe-wide'
                    popup-content-class='flowtag-theme ft-pop'
                />
            </div>

            <div class='spe-row'>
                <span class='spe-label'>BPM</span>
                <div class='ft-field spe-num'><input v-model='draft.bpmMin' type='number' placeholder='min' /></div>
                <span class='spe-dash'>–</span>
                <div class='ft-field spe-num'><input v-model='draft.bpmMax' type='number' placeholder='max' /></div>
            </div>

            <div class='spe-row'>
                <span class='spe-label'>Key</span>
                <q-select
                    v-model='draft.keys' :options='keyOptions' multiple dense options-dense
                    borderless emit-value map-options use-chips class='spe-select spe-wide'
                    popup-content-class='flowtag-theme ft-pop'
                />
            </div>

            <div class='spe-row'>
                <span class='spe-label'>Year</span>
                <div class='ft-field spe-num'><input v-model='draft.yearMin' type='number' placeholder='from' /></div>
                <span class='spe-dash'>–</span>
                <div class='ft-field spe-num'><input v-model='draft.yearMax' type='number' placeholder='to' /></div>
            </div>

            <div class='spe-row'>
                <span class='spe-label'>Added in last</span>
                <div class='ft-field spe-num'><input v-model='draft.addedDays' type='number' placeholder='∞' /></div>
                <span class='spe-unit'>days</span>
            </div>

            <div class='spe-row'>
                <span class='spe-label'>Untagged only</span>
                <q-toggle dense size='sm' v-model='draft.untaggedOnly' />
            </div>
        </div>

        <div class='spe-footer'>
            <span class='spe-count ft-num'>Matches {{ matchCount.toLocaleString() }} track(s)</span>
            <span class='spe-actions'>
                <button type='button' class='ft-btn' @click='emit("close")'>Cancel</button>
                <button type='button' class='ft-btn ft-btn-primary' @click='save'>Save rules</button>
            </span>
        </div>
    </q-card>
</q-dialog>
</template>

<script lang='ts' setup>
import { computed, reactive, watch } from 'vue';
import { get1t } from '../../scripts/onetagger';
import { getTagStore, FlowPlaylist, SavedFilter, parseSavedFilter } from '../../scripts/tagstore';
import { diag } from '../../scripts/diag';

const props = defineProps<{ playlist?: FlowPlaylist }>();
const emit = defineEmits<{ (e: 'close'): void }>();

const $1t = get1t();
const store = getTagStore();

/** Editable mirror of SavedFilter (numbers as strings for the inputs) */
const draft = reactive({
    textInput: '',
    tagIds: [] as number[],
    tagMode: 'all' as 'all' | 'any',
    excludeTagIds: [] as number[],
    bpmMin: '', bpmMax: '',
    keys: [] as string[],
    yearMin: '', yearMax: '',
    addedDays: '',
    untaggedOnly: false,
});

watch(() => props.playlist, (pl) => {
    const f = pl?.filter ? parseSavedFilter(pl.filter) : {};
    draft.textInput = f.text ?? '';
    draft.tagIds = [...(f.tagIds ?? [])];
    draft.tagMode = f.tagMode ?? 'all';
    draft.excludeTagIds = [...(f.excludeTagIds ?? [])];
    draft.bpmMin = f.bpmMin?.toString() ?? '';
    draft.bpmMax = f.bpmMax?.toString() ?? '';
    draft.keys = [...(f.keys ?? [])];
    draft.yearMin = f.yearMin?.toString() ?? '';
    draft.yearMax = f.yearMax?.toString() ?? '';
    draft.addedDays = f.addedDays?.toString() ?? '';
    draft.untaggedOnly = !!f.untaggedOnly;
}, { immediate: true });

const tagOptions = computed(() =>
    [...store.tags.value].sort((a, b) => b.count - a.count).map((t) => ({ label: t.name, value: t.id })));

const keyOptions = computed(() => {
    const seen = new Set<string>();
    for (const t of store.tracks.value) {
        if (t.key) seen.add(t.key.trim().toUpperCase());
    }
    return [...seen].sort((a, b) => a.localeCompare(b, undefined, { numeric: true }))
        .map((k) => ({ label: k, value: k }));
});

const num = (v: string): number | undefined => {
    const n = parseInt(v, 10);
    return Number.isFinite(n) ? n : undefined;
};

const asFilter = computed<SavedFilter>(() => ({
    text: draft.textInput.trim() || undefined,
    tagIds: draft.tagIds.length ? [...draft.tagIds] : undefined,
    tagMode: draft.tagIds.length > 1 && draft.tagMode === 'any' ? 'any' : undefined,
    excludeTagIds: draft.excludeTagIds.length ? [...draft.excludeTagIds] : undefined,
    bpmMin: num(draft.bpmMin),
    bpmMax: num(draft.bpmMax),
    keys: draft.keys.length ? [...draft.keys] : undefined,
    yearMin: num(draft.yearMin),
    yearMax: num(draft.yearMax),
    addedDays: num(draft.addedDays),
    untaggedOnly: draft.untaggedOnly || undefined,
}));

const matchCount = computed(() => {
    const f = asFilter.value;
    const now = Date.now();
    return store.tracks.value.reduce((n, t) => n + (store.passesSaved(t, f, now) ? 1 : 0), 0);
});

function save() {
    if (!props.playlist) return;
    $1t.send('playlistSetFilter', { id: props.playlist.id, filter: JSON.stringify(asFilter.value) });
    diag('playlistRulesSaved', { id: props.playlist.id, matches: matchCount.value });
    emit('close');
}
</script>

<style lang='scss' scoped>
.spe-card {
    width: 520px;
    max-width: 94vw;
    padding: 18px 22px 16px;
}
.spe-sub {
    font-size: 11.5px;
    color: var(--ft-text-3);
    margin: 4px 0 12px;
}
.spe-body {
    display: flex;
    flex-direction: column;
    gap: 9px;
}
.spe-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 30px;
}
.spe-label {
    width: 104px;
    flex-shrink: 0;
    font-size: 12px;
    color: var(--ft-text-2);
}
.spe-wide { flex: 1; min-width: 0; }
.spe-num {
    width: 86px;
    input { width: 100%; }
}
.spe-dash, .spe-unit { color: var(--ft-text-3); font-size: 12px; }
.spe-select :deep(.q-field__control) { min-height: 28px; padding: 0 4px; }
.spe-select :deep(.q-field__control)::before, .spe-select :deep(.q-field__control)::after { display: none; }
.spe-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 16px;
}
.spe-count { font-size: 12px; color: var(--ft-text-2); }
.spe-actions { display: inline-flex; gap: 8px; }
.seg {
    display: inline-flex;
    background: var(--ft-panel);
    border: 1px solid var(--ft-line);
    border-radius: 6px;
    padding: 2px;
    .seg-btn {
        border: none;
        background: transparent;
        color: var(--ft-text-3);
        font-size: 11px;
        font-weight: 500;
        padding: 2px 8px;
        border-radius: 4px;
        cursor: pointer;
        &.on { background: var(--ft-raised); color: var(--ft-text); }
    }
}
</style>

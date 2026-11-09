<template>
<q-dialog :model-value='modelValue' @update:model-value='emit("update:modelValue", $event)'>
    <q-card flat class='export-card flowtag-theme'>
        <div class='ft-title'>Export to comments</div>
        <div class='export-sub'>
            Appends tags as #hashtags to each file's comment field (searchable in
            Rekordbox). Existing comment text is kept; re-exporting updates the block.
        </div>

        <div class='ft-label section'>Scope</div>
        <div class='scope-row' :class='{ on: scope === "all" }' @click='scope = "all"'>
            <q-icon name='mdi-music-box-multiple-outline' size='14px' />
            <span>Whole library</span>
            <span class='scope-count ft-num'>{{ store.tracks.value.length.toLocaleString() }}</span>
        </div>
        <div class='scope-row' :class='{ on: scope === "playlist", dim: store.playlists.value.length === 0 }' @click='pickPlaylist'>
            <q-icon name='mdi-playlist-music' size='14px' />
            <span>Playlist</span>
            <q-select
                v-if='store.playlists.value.length > 0'
                v-model='playlistId'
                :options='playlistOptions'
                dense options-dense borderless emit-value map-options
                class='scope-select'
                popup-content-class='flowtag-theme ft-pop'
                @update:model-value='scope = "playlist"'
                @click.stop
            />
            <span v-else class='scope-count'>none yet</span>
        </div>
        <div class='scope-row' :class='{ on: scope === "tag" }' @click='pickTag'>
            <q-icon name='mdi-tag-outline' size='14px' />
            <span>Tag</span>
            <q-select
                v-model='tagId'
                :options='tagOptions'
                dense options-dense borderless emit-value map-options
                class='scope-select'
                popup-content-class='flowtag-theme ft-pop'
                @update:model-value='scope = "tag"'
                @click.stop
            />
        </div>

        <div class='export-actions'>
            <button type='button' class='ft-btn' @click='emit("update:modelValue", false)'>Cancel</button>
            <button type='button' class='ft-btn ft-btn-primary' :disabled='exportCount === 0' @click='run'>
                Export {{ exportCount.toLocaleString() }} track(s)
            </button>
        </div>
    </q-card>
</q-dialog>
</template>

<script lang='ts' setup>
import { computed, ref } from 'vue';
import { notify } from '@/lib/notify';

import { get1t } from '../../scripts/onetagger';
import { getTagStore } from '../../scripts/tagstore';
import { diag } from '../../scripts/diag';

const props = defineProps<{ modelValue: boolean }>();
const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>();

const $1t = get1t();
const store = getTagStore();

const scope = ref<'all' | 'playlist' | 'tag'>('all');
const playlistId = ref<number | undefined>(store.activePlaylistId.value);
const tagId = ref<number | undefined>(undefined);

const playlistOptions = computed(() =>
    store.playlists.value.map((p) => ({ label: p.name, value: p.id })));
const tagOptions = computed(() =>
    [...store.tags.value].sort((a, b) => b.count - a.count).map((t) => ({ label: `${t.name} (${t.count})`, value: t.id })));

function pickPlaylist() {
    if (store.playlists.value.length === 0) return;
    if (playlistId.value === undefined) playlistId.value = store.playlists.value[0].id;
    scope.value = 'playlist';
}
function pickTag() {
    if (tagId.value === undefined && store.tags.value.length > 0) tagId.value = tagOptions.value[0].value;
    scope.value = 'tag';
}

/** undefined = whole library (server exports every present track) */
const scopedTrackIds = computed<number[] | undefined>(() => {
    if (scope.value === 'playlist') {
        const pl = store.playlists.value.find((p) => p.id === playlistId.value);
        return pl ? store.playlistTrackIds(pl) : [];
    }
    if (scope.value === 'tag') {
        if (tagId.value === undefined) return [];
        return store.tracks.value.filter((t) => t.tagIds.includes(tagId.value!)).map((t) => t.id);
    }
    return undefined;
});

const exportCount = computed(() =>
    scopedTrackIds.value === undefined ? store.tracks.value.length : scopedTrackIds.value.length);

function run() {
    $1t.send('rekordboxExport', { mode: 'comment', trackIds: scopedTrackIds.value ?? null });
    diag('exportComments', { scope: scope.value, tracks: exportCount.value });
    notify({ message: 'Export started…', timeout: 1500 });
    emit('update:modelValue', false);
}
</script>

<style lang='scss' scoped>
.export-card {
    width: 440px;
    max-width: 92vw;
    padding: 18px 22px 16px;
}
.export-sub {
    font-size: 12px;
    color: var(--ft-text-3);
    line-height: 1.5;
    margin: 6px 0 2px;
}
.section { margin: 14px 0 6px; }
.scope-row {
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 34px;
    padding: 0 10px;
    border-radius: 6px;
    font-size: 12.5px;
    color: var(--ft-text-2);
    cursor: default;

    .q-icon { color: var(--ft-text-3); }
    &:hover { background: var(--ft-hover); }
    &.on { background: var(--ft-raised); color: var(--ft-text); box-shadow: inset 0 0 0 1px var(--ft-line-strong); }
    &.dim { opacity: 0.55; }
    .scope-count { margin-left: auto; font-size: 11px; color: var(--ft-text-3); }
    .scope-select { margin-left: auto; min-width: 150px; }
}
.export-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
}
</style>

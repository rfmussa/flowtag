<template>
<Dialog :open='modelValue' @update:open='emit("update:modelValue", $event)'>
    <DialogContent class='sm:max-w-[440px] gap-0 px-[22px] pt-[18px] pb-4'>
        <DialogHeader>
            <DialogTitle>Export to comments</DialogTitle>
            <DialogDescription class='mt-1.5'>
                Appends tags as #hashtags to each file's comment field (searchable in
                Rekordbox). Existing comment text is kept; re-exporting updates the block.
            </DialogDescription>
        </DialogHeader>

        <div class='ft-label section'>Scope</div>
        <div class='scope-row' :class='{ on: scope === "all" }' @click='scope = "all"'>
            <LibraryIcon class='scope-icon' />
            <span>Whole library</span>
            <span class='scope-count ft-num'>{{ store.tracks.value.length.toLocaleString() }}</span>
        </div>
        <div class='scope-row' :class='{ on: scope === "playlist", dim: store.playlists.value.length === 0 }' @click='pickPlaylist'>
            <ListMusicIcon class='scope-icon' />
            <span>Playlist</span>
            <OptionSelect
                v-if='store.playlists.value.length > 0'
                v-model='playlistId'
                :options='playlistOptions'
                label='Playlist'
                placeholder='Pick a playlist'
                size='sm'
                trigger-class='ml-auto min-w-[150px]'
                @update:model-value='scope = "playlist"'
            />
            <span v-else class='scope-count'>none yet</span>
        </div>
        <div class='scope-row' :class='{ on: scope === "tag" }' @click='pickTag'>
            <TagIcon class='scope-icon' />
            <span>Tag</span>
            <OptionSelect
                v-model='tagId'
                :options='tagOptions'
                label='Tag'
                placeholder='Pick a tag'
                size='sm'
                trigger-class='ml-auto min-w-[150px]'
                @update:model-value='scope = "tag"'
            />
        </div>

        <DialogFooter class='mt-4'>
            <Button @click='emit("update:modelValue", false)'>Cancel</Button>
            <Button variant='default' :disabled='exportCount === 0' @click='run'>
                Export {{ exportCount.toLocaleString() }} track(s)
            </Button>
        </DialogFooter>
    </DialogContent>
</Dialog>
</template>

<script lang='ts' setup>
import { computed, ref } from 'vue';
import { LibraryIcon, ListMusicIcon, TagIcon } from '@lucide/vue';
import { notify } from '@/lib/notify';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import OptionSelect from './OptionSelect.vue';

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

    .scope-icon { width: 14px; height: 14px; color: var(--ft-text-3); flex-shrink: 0; }
    &:hover { background: var(--ft-hover); }
    &.on { background: var(--ft-raised); color: var(--ft-text); box-shadow: inset 0 0 0 1px var(--ft-line-strong); }
    &.dim { opacity: 0.55; }
    .scope-count { margin-left: auto; font-size: 11px; color: var(--ft-text-3); }
}
</style>

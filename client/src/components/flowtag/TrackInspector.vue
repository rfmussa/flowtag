<template>
<Dialog v-model:open='open'>
    <DialogContent class='track-inspector sm:max-w-[460px] gap-0 p-0' :show-close-button='false'>
        <div class='insp-head flex items-center'>
            <DialogTitle>Edit metadata</DialogTitle>
            <DialogDescription class='sr-only'>Edit the file tags of the selected track</DialogDescription>
            <div class='flex-1'></div>
            <div class='insp-path ft-mono truncate' :title='track?.path'>{{ track?.path }}</div>
        </div>

        <div v-if='loading' class='insp-loading flex justify-center items-center'>
            <LoaderCircleIcon class='size-[22px] animate-spin text-ft-text-3' aria-label='Loading metadata' />
        </div>

        <form v-else class='insp-body' @submit.prevent='save'>
            <label class='ft-label'>Title</label>
            <label class='ft-field'><input ref='titleEl' v-model='draft.title' type='text' spellcheck='false' /></label>

            <label class='ft-label'>Artist</label>
            <label class='ft-field'><input v-model='draft.artist' type='text' spellcheck='false' /></label>

            <label class='ft-label'>Album</label>
            <label class='ft-field'><input v-model='draft.album' type='text' spellcheck='false' /></label>

            <div class='insp-row'>
                <div class='insp-cell'>
                    <label class='ft-label'>Year</label>
                    <label class='ft-field'><input v-model='yearText' type='text' inputmode='numeric' class='ft-num' /></label>
                </div>
                <div class='insp-cell'>
                    <label class='ft-label'>BPM</label>
                    <label class='ft-field'><input v-model='bpmText' type='text' inputmode='numeric' class='ft-num' /></label>
                </div>
                <div class='insp-cell'>
                    <label class='ft-label'>Key</label>
                    <label class='ft-field'><input v-model='draft.key' type='text' spellcheck='false' placeholder='8A / Am' /></label>
                </div>
            </div>

            <label class='ft-label'>Genre</label>
            <label class='ft-field'><input v-model='draft.genre' type='text' spellcheck='false' /></label>

            <label class='ft-label'>Comment</label>
            <label class='ft-field'><input v-model='draft.comment' type='text' spellcheck='false' /></label>

            <div class='insp-actions flex items-center'>
                <span class='insp-hint'><span class='ft-kbd'>⌘↵</span> save</span>
                <div class='flex-1'></div>
                <Button @click='open = false'>Cancel</Button>
                <Button type='submit' variant='default' :disabled='saving'>
                    {{ saving ? 'Saving…' : 'Save' }}
                </Button>
            </div>
        </form>
    </DialogContent>
</Dialog>
</template>

<script lang='ts' setup>
import { LoaderCircleIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog';
import { computed, nextTick, ref, watch } from 'vue';
import { notify } from '@/lib/notify';
import { useEventListener } from '@vueuse/core';

import { get1t } from '../../scripts/onetagger';
import { FlowTrack, getTagStore, TrackMeta } from '../../scripts/tagstore';
import { diag } from '../../scripts/diag';

const $1t = get1t();
const store = getTagStore();

const open = ref(false);
const loading = ref(true);
const saving = ref(false);
const track = ref<FlowTrack | undefined>();
const titleEl = ref<HTMLInputElement>();

const emptyMeta = (): TrackMeta =>
    ({ title: '', artist: '', album: '', genre: '', year: null, bpm: null, key: '', comment: '' });
const draft = ref<TrackMeta>(emptyMeta());
const yearText = ref('');
const bpmText = ref('');
/** Restart playback after save when the save had to stop this track */
let resumeAfterSave = false;

// E opens the inspector for the selected track; metadata is read fresh from
// the file so the form shows the file's truth, not the DB cache
watch(store.inspectorSignal, () => {
    const t = store.selectedTrack();
    if (!t || t.missing) return;
    track.value = t;
    draft.value = emptyMeta();
    loading.value = true;
    saving.value = false;
    open.value = true;
    $1t.send('trackMetaGet', { trackId: t.id });
});

watch(store.inspectorMeta, (v) => {
    if (!v || !open.value || v.trackId !== track.value?.id) return;
    draft.value = { ...emptyMeta(), ...v.meta };
    yearText.value = v.meta.year != null ? String(v.meta.year) : '';
    bpmText.value = v.meta.bpm != null ? String(v.meta.bpm) : '';
    loading.value = false;
    nextTick(() => titleEl.value?.focus());
});

const parsedYear = computed(() => parseIntOrNull(yearText.value));
const parsedBpm = computed(() => parseIntOrNull(bpmText.value));
function parseIntOrNull(s: string): number | null {
    const n = parseInt(s.trim(), 10);
    return Number.isFinite(n) ? n : null;
}

function save() {
    const t = track.value;
    if (!t || saving.value || loading.value) return;
    diag('metaSave', { trackId: t.id });
    saving.value = true;
    // Release the file before the write: the selected track is usually the one
    // loaded in the player
    const player = $1t.player.value;
    resumeAfterSave = player.playing && player.path === t.path;
    if (player.path === t.path) player.stop();
    const meta: TrackMeta = { ...draft.value, year: parsedYear.value, bpm: parsedBpm.value };
    $1t.send('trackMetaSet', { trackId: t.id, meta });
    // A failed save surfaces as the app's error toast; don't leave the dialog stuck
    setTimeout(() => { saving.value = false; }, 5000);
}

// Save confirmed by the server echo: close, and resume playback from 0
watch(store.metaSavedSignal, () => {
    if (!open.value || !saving.value) return;
    saving.value = false;
    open.value = false;
    const t = track.value;
    notify({ message: 'Metadata saved', timeout: 1500 });
    if (t && resumeAfterSave) {
        const fresh = store.tracks.value.find((x) => x.id === t.id);
        $1t.player.value.startTrack(t.path, t.id, fresh?.title ?? t.title, fresh?.artist ?? t.artist, true);
    }
});

function onKey(e: KeyboardEvent) {
    if (e.code === 'Enter' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        save();
    }
}
// Attached only while the inspector is open; detached on unmount too
useEventListener(() => (open.value ? document : null), 'keydown', onKey);
</script>

<style lang='scss'>
.track-inspector {
    .insp-head {
        padding: 14px 16px 10px;
        gap: 12px;
        .insp-path { max-width: 55%; color: var(--ft-text-3); direction: rtl; text-align: right; }
    }

    .insp-loading { height: 280px; }

    .insp-body {
        padding: 0 16px 14px;

        .ft-label { display: block; margin: 10px 0 4px; }
        .ft-field { background: var(--ft-panel); }

        .insp-row {
            display: flex;
            gap: 10px;
            .insp-cell { flex: 1; min-width: 0; }
        }

        .insp-actions {
            margin-top: 16px;
            gap: 8px;
            .insp-hint { font-size: 11px; color: var(--ft-text-3); }
        }
    }
}
</style>

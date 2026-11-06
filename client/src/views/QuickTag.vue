<template>
<div class='flowtag column no-wrap'>

    <!-- Header bar -->
    <div class='row items-center q-px-md q-py-sm header-bar no-wrap'>
        <FolderManager />
        <ScanStatus class='q-ml-md' />
        <q-space />
        <FlushIndicator class='q-mr-md' />
        <q-input
            v-model='store.filter.value'
            dense outlined clearable
            debounce='50'
            placeholder='Filter tracks or tags'
            class='filter-input q-mr-md'
        />
        <q-toggle v-model='store.untaggedOnly.value' label='Untagged only' dense class='q-mr-md' />
        <q-btn dense flat no-caps icon='mdi-tag-multiple' label='Manage' class='q-px-sm' @click='openManager'></q-btn>
        <q-btn dense flat no-caps icon='mdi-export' label='Export' class='q-px-sm' @click='confirmExport'>
            <q-tooltip>Write tags as #hashtags into each file's comment (Rekordbox-searchable)</q-tooltip>
        </q-btn>
        <q-btn
            dense flat round
            class='q-ml-xs'
            :icon='paletteOpen ? "mdi-dock-right" : "mdi-dock-left"'
            @click='togglePalette'
        >
            <q-tooltip>{{ paletteOpen ? 'Hide tag palette' : 'Show tag palette' }}</q-tooltip>
        </q-btn>
    </div>

    <!-- Track list + palette -->
    <div class='col row no-wrap list-row'>
    <div class='col list-area'>
        <div v-if='store.folders.value.length === 0 && store.loaded.value' class='column items-center justify-center full-height text-grey-5'>
            <q-icon name='mdi-music-box-multiple-outline' size='64px' class='q-mb-md' />
            <div class='text-h6'>No music folders yet</div>
            <div class='q-mb-md'>Add a folder and scan it to start tagging</div>
            <FolderManager />
        </div>
        <div v-else-if='!store.loaded.value' class='column items-center justify-center full-height'>
            <q-circular-progress indeterminate color='primary' size='48px' />
        </div>
        <TrackList v-else />
    </div>
    <div v-if='paletteOpen' class='palette-col'>
        <TagPalette />
    </div>
    </div>

    <!-- Tagging panel -->
    <div class='tag-panel q-px-md q-py-sm'>
        <div v-if='selected' class='row items-center no-wrap'>
            <div class='track-meta q-mr-md'>
                <div class='text-weight-bold ellipsis'>{{ selected.title }}</div>
                <div class='text-caption text-grey-6 ellipsis'>{{ selected.artist }}</div>
            </div>
            <TagChips :tag-ids='selected.tagIds' :track-id='selected.id' removable class='q-mr-md chips' />
            <TagInput :track-id='selected.id' />
        </div>
        <div v-else class='text-grey-6 text-center q-py-xs'>
            Select a track (<b>J</b>/<b>K</b> or click), press <b>T</b> to tag, <b>D</b> when done — <b>Space</b> plays
        </div>
    </div>

    <TagManager v-model='managerOpen' />
</div>
</template>

<script lang='ts' setup>
import { computed, onMounted, ref, watch } from 'vue';
import { Dialog, Notify } from 'quasar';
import { get1t } from '../scripts/onetagger';
import { getTagStore } from '../scripts/tagstore';
import FolderManager from '../components/flowtag/FolderManager.vue';
import FlushIndicator from '../components/flowtag/FlushIndicator.vue';
import ScanStatus from '../components/flowtag/ScanStatus.vue';
import TrackList from '../components/flowtag/TrackList.vue';
import TagChips from '../components/flowtag/TagChips.vue';
import TagInput from '../components/flowtag/TagInput.vue';
import TagPalette from '../components/flowtag/TagPalette.vue';
import TagManager from '../components/flowtag/TagManager.vue';

const PALETTE_KEY = 'flowtag.paletteOpen';

const $1t = get1t();
const store = getTagStore();

const selected = computed(() => {
    store.tracks.value; // reactive dep
    return store.selectedTrack();
});

const managerOpen = ref(false);
const paletteOpen = ref(readPaletteOpen());

function readPaletteOpen(): boolean {
    try {
        return localStorage.getItem(PALETTE_KEY) !== 'false';
    } catch {
        return true;
    }
}

function blurTarget(e: Event) {
    (e.currentTarget as HTMLElement | null)?.blur();
}

function openManager(e: Event) {
    blurTarget(e);
    managerOpen.value = true;
}

function confirmExport(e: Event) {
    blurTarget(e);
    Dialog.create({
        title: 'Export to comments',
        message: 'Append your tags as #hashtags to every tagged file\'s comment field (searchable in Rekordbox). Existing comment text is kept; re-exporting updates the hashtag block. Continue?',
        cancel: true,
        dark: true,
    }).onOk(() => {
        $1t.send('rekordboxExport', { mode: 'comment' });
        Notify.create({ message: 'Export started…', timeout: 1500 });
    });
}

function togglePalette(e: Event) {
    blurTarget(e);
    paletteOpen.value = !paletteOpen.value;
    try {
        localStorage.setItem(PALETTE_KEY, String(paletteOpen.value));
    } catch {
        // Storage unavailable: keep the in-memory state only
    }
}

// Selecting a track loads it into the player (playback from 0);
// playback continues across switches when it was already running
watch(store.selectedId, (id) => {
    const track = store.selectedTrack();
    if (id === undefined || !track || track.missing) return;
    const player = $1t.player.value;
    const autoplay = player.playing || player.wasPlaying;
    player.startTrack(track.path, track.id, track.title, track.artist, autoplay);
    // Hint the server to flush the previously edited track soon
    $1t.send('flushHint', {});
});

onMounted(() => {
    if (!store.loaded.value) store.load();
    $1t.send('flushStatusGet');
});
</script>

<style lang='scss' scoped>
.flowtag {
    height: calc(100vh - 100px);
}
.header-bar {
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
    flex-shrink: 0;
}
.filter-input {
    width: 260px;
}
.list-row {
    min-height: 0;
}
.list-area {
    min-height: 0;
    min-width: 0;
    overflow: hidden;
}
.palette-col {
    width: 230px;
    flex-shrink: 0;
    border-left: 1px solid rgba(255, 255, 255, 0.07);
    min-height: 0;
    overflow: hidden;
}
.tag-panel {
    border-top: 1px solid rgba(255, 255, 255, 0.07);
    flex-shrink: 0;
    min-height: 56px;

    .track-meta {
        width: 220px;
        min-width: 120px;
    }
    .chips {
        max-width: 40%;
        overflow: hidden;
    }
}
.full-height {
    height: 100%;
}
</style>

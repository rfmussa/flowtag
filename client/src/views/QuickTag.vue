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
        <q-toggle v-model='store.untaggedOnly.value' label='Untagged only' dense />
    </div>

    <!-- Track list / empty states -->
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
</div>
</template>

<script lang='ts' setup>
import { computed, onMounted, watch } from 'vue';
import { get1t } from '../scripts/onetagger';
import { getTagStore } from '../scripts/tagstore';
import FolderManager from '../components/flowtag/FolderManager.vue';
import FlushIndicator from '../components/flowtag/FlushIndicator.vue';
import ScanStatus from '../components/flowtag/ScanStatus.vue';
import TrackList from '../components/flowtag/TrackList.vue';
import TagChips from '../components/flowtag/TagChips.vue';
import TagInput from '../components/flowtag/TagInput.vue';

const $1t = get1t();
const store = getTagStore();

const selected = computed(() => {
    store.tracks.value; // reactive dep
    return store.selectedTrack();
});

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
.list-area {
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

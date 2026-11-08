<template>
<div class='mini flowtag-theme'>
    <!-- Top: art + track info (drag region) + transport -->
    <div class='mini-top' @mousedown='onDrag'>
        <img v-if='np' :src='art' class='mini-art' alt='' draggable='false' />
        <div v-else class='mini-art mini-art-empty'></div>
        <div class='mini-info'>
            <div class='mini-title'>{{ np?.title || 'Nothing playing' }}</div>
            <div class='mini-artist'>{{ np?.artist || 'Play a track in the main window' }}</div>
        </div>
        <div class='mini-transport' @mousedown.stop>
            <button type='button' class='ft-btn icon-only' aria-label='Previous' @click='remote("prev")'>
                <q-icon name='mdi-skip-previous' size='17px' />
            </button>
            <button type='button' class='ft-btn icon-only' aria-label='Play/pause' @click='remote("playPause")'>
                <q-icon :name='np?.playing ? "mdi-pause" : "mdi-play"' size='19px' />
            </button>
            <button type='button' class='ft-btn icon-only' aria-label='Next' @click='remote("next")'>
                <q-icon name='mdi-skip-next' size='17px' />
            </button>
        </div>
        <button type='button' class='ft-btn icon-only mini-close' aria-label='Close miniplayer' @mousedown.stop @click='closeMini'>
            <q-icon name='mdi-close' size='14px' />
        </button>
    </div>

    <!-- Progress -->
    <div class='mini-progress'>
        <div class='mini-progress-fill' :style='{ width: progress + "%" }'></div>
    </div>

    <!-- Tagging: the playing track's chips + input -->
    <div class='mini-tags'>
        <TagChips v-if='track' :tagIds='track.tagIds' :trackId='track.id' removable dense class='mini-chips' />
        <span v-else class='mini-chips-empty'></span>
        <span class='mini-time ft-num'>{{ fmt(displayPos) }} / {{ fmt(np?.durationMs ?? 0) }}</span>
    </div>
    <div class='mini-input'>
        <TagInput :trackId='track?.id' />
    </div>
</div>
</template>

<script lang='ts' setup>
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { get1t } from '../scripts/onetagger';
import { diag } from '../scripts/diag';
import { getTagStore } from '../scripts/tagstore';
import { Keybind, httpUrl, PLACEHOLDER_IMG } from '../scripts/utils';
import { FLOWTAG_ACTION_DEFAULTS } from '../scripts/flowkeys';
import { FlowTagAction } from '../scripts/settings';
import TagChips from '../components/flowtag/TagChips.vue';
import TagInput from '../components/flowtag/TagInput.vue';

const $1t = get1t();
const store = getTagStore();

const np = computed(() => store.nowPlaying.value);
const track = computed(() => {
    const path = np.value?.path;
    if (!path) return undefined;
    return store.tracks.value.find((t) => t.path === path);
});
const art = computed(() => np.value?.path
    ? `${httpUrl()}/thumb?path=${encodeURIComponent(np.value.path)}`
    : PLACEHOLDER_IMG);

// Position interpolates between snapshots while playing
const clock = ref(Date.now());
let timer: number | undefined;
const displayPos = computed(() => {
    const n = np.value;
    if (!n) return 0;
    const elapsed = n.playing ? clock.value - n.at : 0;
    return Math.min(n.positionMs + elapsed, n.durationMs);
});
const progress = computed(() => {
    const n = np.value;
    if (!n || !n.durationMs) return 0;
    return Math.min((displayPos.value / n.durationMs) * 100, 100);
});

function fmt(ms: number): string {
    const s = Math.max(Math.round(ms / 1000), 0);
    return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, '0')}`;
}

/** Transport goes through the server broadcast; the main window executes */
function remote(key: string) {
    diag('miniRemote', { key });
    $1t.send('remoteCommand', { key });
}

// Frameless window: background drag + close via the shell's IPC
function ipc(message: string) {
    try {
        // @ts-ignore
        window.ipc.postMessage(message);
    } catch (e) { /* not inside the webview shell */ }
}
function onDrag() { ipc('dragMini'); }
function closeMini() { ipc('closeMini'); }

/** Mini keyboard: transport + tagging for the PLAYING track.
 *  Custom binds from settings apply; fixed keys mirror the main window. */
function matches(e: KeyboardEvent, action: FlowTagAction): boolean {
    const kb = $1t.settings.value.flowtag.keybinds[action];
    if (kb && kb.key) return !!Keybind.check(e, Keybind.fromJson(kb));
    return e.code === FLOWTAG_ACTION_DEFAULTS[action]
        && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey;
}

function onKey(e: KeyboardEvent) {
    const target = e.target as HTMLElement | null;
    if (target && (target.nodeName === 'INPUT' || target.nodeName === 'TEXTAREA')) return;
    let handled = true;
    if (e.code === 'Space') remote('playPause');
    else if (e.code === 'ArrowUp' || matches(e, 'prev')) remote('prev');
    else if (e.code === 'ArrowDown' || matches(e, 'next')) remote('next');
    else if (matches(e, 'focusTag')) {
        store.focusInputSignal.value++;
    } else if (matches(e, 'done') && track.value) {
        $1t.send('flushHint', { trackId: track.value.id });
        remote('next');
    } else {
        handled = false;
        // Per-tag hotkeys toggle on the playing track
        if (track.value) {
            for (const tag of store.tags.value) {
                if (tag.keybind && Keybind.check(e, Keybind.fromJson(tag.keybind))) {
                    store.toggleTag(track.value.id, tag);
                    handled = true;
                    break;
                }
            }
        }
    }
    if (handled) e.preventDefault();
}

onMounted(() => {
    diag('miniOpen');
    if (!store.loaded.value) store.load();
    window.addEventListener('keydown', onKey);
    timer = window.setInterval(() => { clock.value = Date.now(); }, 500);
});
onUnmounted(() => {
    window.removeEventListener('keydown', onKey);
    if (timer !== undefined) window.clearInterval(timer);
});
</script>

<style lang='scss' scoped>
.mini {
    display: flex;
    flex-direction: column;
    height: 100vh;
    box-sizing: border-box;
    background: var(--ft-bg);
    border: 1px solid var(--ft-line-strong);
    border-radius: 10px;
    overflow: hidden;
    user-select: none;
}
.mini-top {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 10px 8px 10px;
    cursor: default;
}
.mini-art {
    width: 44px;
    height: 44px;
    border-radius: 6px;
    object-fit: cover;
    flex-shrink: 0;
    pointer-events: none;
}
.mini-art-empty {
    background: var(--ft-panel);
    border: 1px solid var(--ft-line);
}
.mini-info {
    flex: 1;
    min-width: 0;
}
.mini-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--ft-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.mini-artist {
    font-size: 11.5px;
    color: var(--ft-text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.mini-transport {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
}
.mini-close {
    flex-shrink: 0;
    align-self: flex-start;
    margin: -4px -4px 0 0;
}
.mini-progress {
    height: 2px;
    background: var(--ft-line);
    margin: 0 10px;
    border-radius: 1px;
}
.mini-progress-fill {
    height: 100%;
    background: var(--ft-text-2);
    border-radius: 1px;
    transition: width 500ms linear;
}
.mini-tags {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 10px 0 10px;
    min-height: 26px;
}
.mini-chips {
    overflow: hidden;
    min-width: 0;
}
.mini-time {
    font-size: 10.5px;
    color: var(--ft-text-3);
    flex-shrink: 0;
}
.mini-input {
    padding: 6px 10px 10px 10px;
}
</style>

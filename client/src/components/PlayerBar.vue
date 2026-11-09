<template>
<div>

    <!-- Compact: slim strip, waveform hidden -->
    <div v-if='compact' class='compact-row'>
        <div class='compact-progress' :style='{ width: progress + "%" }'></div>
        <Button
            ref='playButton'
            size='icon-sm'
            :aria-label='$1t.player.value.playing ? "Pause" : "Play"'
            @click='$1t.player.value.playing ? $1t.player.value.pause() : $1t.player.value.play()'
        >
            <PauseIcon v-if='$1t.player.value.playing' /><PlayIcon v-else />
        </Button>
        <span class='compact-title' v-if='$1t.player.value.title'>{{ $1t.player.value.title }}</span>
        <span class='compact-artist' v-if='$1t.player.value.artists?.length'>{{ $1t.player.value.artists.join(', ') }}</span>
        <span class='compact-spacer'></span>
        <span class='monospace'>{{ fmt($1t.player.value.position) }} / {{ fmt($1t.player.value.duration) }}</span>
        <Button size='icon' title='Expand player' aria-label='Expand player' @click='toggleCompact'><ChevronUpIcon /></Button>
    </div>

    <!-- Full bar -->
    <div v-else class='mx-4 grid grid-cols-12'>
        <!-- Meta -->
        <div class='col-span-2 flex min-w-0'>
            <!-- Album art, with the placeholder when the file has none -->
            <img
                :src='artFailed ? PLACEHOLDER_IMG : art'
                alt=''
                class='mt-2 size-[46px] shrink-0 rounded object-cover'
                @error='artFailed = true'
            />
            <div class='mt-2.5 flex min-w-0 flex-col pl-2 text-xs'>
                <div v-if='$1t.player.value.title' class='truncate font-bold text-ft-text'>{{ $1t.player.value.title }}</div>
                <div v-if='$1t.player.value.artists' class='truncate text-ft-text-3'>{{ $1t.player.value.artists.join(', ') }}</div>
            </div>
        </div>

        <div class='col-span-8'>
            <div class='flex justify-center'>
                <div class='mt-2 mr-2'>
                    <Button
                        ref='playButton'
                        size='icon-lg'
                        class='rounded-full'
                        :aria-label='$1t.player.value.playing ? "Pause" : "Play"'
                        @click='$1t.player.value.playing ? $1t.player.value.pause() : $1t.player.value.play()'
                    >
                        <PauseIcon v-if='$1t.player.value.playing' class='size-5' /><PlayIcon v-else class='size-5' />
                    </Button>
                </div>
                <div><Waveform></Waveform></div>
            </div>
        </div>

        <div class='col-span-2 flex items-center justify-end'>
            <span
                v-if='$1t.settings.value.flowtag.analysisEnabled && $1t.player.value.gainDb'
                class='gain-badge ft-num'
                title='Auto-gain: matched to -14 LUFS'
            >{{ $1t.player.value.gainDb > 0 ? '+' : '' }}{{ $1t.player.value.gainDb.toFixed(1) }} dB</span>
            <!-- Volume: applied while dragging, saved when released -->
            <Slider
                class='w-[88px]'
                :min='0'
                :max='1'
                :step='0.01'
                :model-value='[$1t.player.value.volume]'
                aria-label='Volume'
                @update:model-value='(v) => setVolume(v?.[0])'
                @value-commit='$1t.saveSettings(false)'
            />
            <Button size='icon' class='ml-2' title='Compact player' aria-label='Compact player' @click='toggleCompact'><ChevronDownIcon /></Button>
        </div>
    </div>

</div>
</template>

<script lang='ts' setup>
import Waveform from './Waveform.vue';
import { ChevronDownIcon, ChevronUpIcon, PauseIcon, PlayIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { Slider } from '@/components/ui/slider';
import { PLACEHOLDER_IMG, httpUrl } from '../scripts/utils';
import { computed, onDeactivated, onMounted, ref, watch } from 'vue';
import { useLocalStorage } from '@vueuse/core';
import { get1t } from '../scripts/onetagger';

const $1t = get1t();
const playButton = ref<any>();

// Compact footer: per-machine convenience, survives restarts. Stored as
// '1'/'0' (the format earlier builds wrote), not VueUse's 'true'/'false'.
const compact = useLocalStorage('flowtag-compact-player', false, {
    serializer: {
        read: (v: string) => v === '1' || v === 'true',
        write: (v: boolean) => (v ? '1' : '0'),
    },
});

/** The track list sizes itself against this (QuickTag: calc(100vh - var(--player-h))) */
watch(compact, (c) => {
    document.documentElement.style.setProperty('--player-h', c ? '36px' : '60px');
}, { immediate: true });

function toggleCompact() {
    compact.value = !compact.value;
}

function fmt(ms: number): string {
    const s = Math.max(Math.round((ms ?? 0) / 1000), 0);
    return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, '0')}`;
}

const progress = computed(() => {
    const d = $1t.player.value.duration;
    if (!d) return 0;
    return Math.min(($1t.player.value.position / d) * 100, 100);
});

onMounted(() => {
    // Unfocus callback (keeps Space from re-triggering the focused play button)
    $1t.quickTagUnfocus = () => {
        playButton.value?.$el?.focus();
        playButton.value?.$el?.blur();
    }
});

onDeactivated(() => $1t.quickTagUnfocus = () => {});

const art = computed(() => `${httpUrl()}/thumb?path=${encodeURIComponent($1t.player.value.path??'')}`);
// Fall back to the placeholder when the file has no artwork; retry per track
const artFailed = ref(false);
watch(art, () => { artFailed.value = false; });

function setVolume(volume?: number) {
    if (volume === undefined) return;
    $1t.player.value.volume = volume;
    $1t.player.value.setVolume(volume);
}

</script>

<style lang='scss' scoped>
.gain-badge {
    font-size: 10.5px;
    color: var(--ft-text-3);
    margin-right: 10px;
}
.compact-row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 36px;
    padding: 0 12px;
    overflow: hidden;
}
.compact-progress {
    position: absolute;
    top: 0;
    left: 0;
    height: 2px;
    background: var(--ft-text-3);
    transition: width 300ms linear;
}
.compact-title {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--ft-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 40%;
}
.compact-artist {
    font-size: 12px;
    color: var(--ft-text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
}
.compact-spacer { flex: 1; }
</style>

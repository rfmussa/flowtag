<template>
<div>

    <!-- Compact: slim strip, waveform hidden -->
    <div v-if='compact' class='compact-row'>
        <div class='compact-progress' :style='{ width: progress + "%" }'></div>
        <q-btn
            round flat dense size='12px'
            :icon='$1t.player.value.playing ? "mdi-pause" : "mdi-play"'
            :ripple='false'
            @click='$1t.player.value.playing ? $1t.player.value.pause() : $1t.player.value.play()'
            ref='playButton'
        ></q-btn>
        <span class='compact-title' v-if='$1t.player.value.title'>{{ $1t.player.value.title }}</span>
        <span class='compact-artist' v-if='$1t.player.value.artists?.length'>{{ $1t.player.value.artists.join(', ') }}</span>
        <span class='compact-spacer'></span>
        <span class='monospace'>{{ fmt($1t.player.value.position) }} / {{ fmt($1t.player.value.duration) }}</span>
        <button type='button' class='ft-btn' title='Expand player' @click='toggleCompact'>
            <q-icon name='mdi-chevron-up' size='15px' />
        </button>
    </div>

    <!-- Full bar -->
    <div v-else class="row q-mx-md">
        <!-- Meta -->
        <div class="row col-2">

            <!-- Album art -->
            <div class='q-mt-sm'>
                <q-img
                    :src='art'
                    width='46px'
                    height='46px'
                    class='rounded-borders'
                    :placeholder-src='PLACEHOLDER_IMG'
                >
                    <template v-slot:error>
                        <q-img :src='PLACEHOLDER_IMG' width='46px' height='46px' class='rounded-borders'></q-img>
                    </template>
                </q-img>
            </div>

            <div class="column q-mt-sm q-pt-xs q-pl-sm" style="width: calc(100% - 50px);">
                <div class="text-caption text-weight-bold full-width">
                    <div v-if="$1t.player.value.title" class="text-no-wrap overflow-hidden" style="text-overflow: ellipsis">
                        {{ $1t.player.value.title }}
                    </div>
                </div>

                <div class="text-caption full-width text-grey-5">
                    <div v-if="$1t.player.value.artists" class="text-no-wrap overflow-hidden" style="text-overflow: ellipsis">
                        {{ $1t.player.value.artists.join(', ') }}
                    </div>
                </div>
            </div>

        </div>

        <div class="col-8">
            <div class='row justify-center'>
                <div class='q-mt-sm'>
                    <!-- Play button -->
                    <q-btn
                        round
                        flat
                        icon="mdi-play"
                        class="q-mr-sm"
                        :ripple="false"
                        v-if="!$1t.player.value.playing"
                        @click="$1t.player.value.play()"
                        ref='playButton'
                    ></q-btn>
                    <!-- Pause -->
                    <q-btn
                        round
                        flat
                        icon="mdi-pause"
                        class="q-mr-sm"
                        :ripple="false"
                        v-else
                        @click="$1t.player.value.pause()"
                        ref='playButton'
                    ></q-btn>
                </div>

                <div><Waveform></Waveform></div>
            </div>
        </div>

        <div class='row col-2 justify-end items-center'>
            <span
                v-if='$1t.settings.value.flowtag.analysisEnabled && $1t.player.value.gainDb'
                class='gain-badge ft-num'
                title='Auto-gain: matched to -14 LUFS'
            >{{ $1t.player.value.gainDb > 0 ? '+' : '' }}{{ $1t.player.value.gainDb.toFixed(1) }} dB</span>
            <!-- Volume -->
            <div class="q-pt-sm" style="width: 88px">
                <q-slider
                    v-model="$1t.player.value.volume"
                    :min="0.0"
                    :max="1.0"
                    :step="0.01"
                    @update:model-value="(v: any) => $1t.player.value.setVolume(v)"
                    @change="$1t.saveSettings(false)"
                    style="margin-top: 6px"
                ></q-slider>
            </div>
            <button type='button' class='ft-btn q-ml-sm' title='Compact player' @click='toggleCompact'>
                <q-icon name='mdi-chevron-down' size='15px' />
            </button>
        </div>
    </div>

</div>
</template>

<script lang='ts' setup>
import Waveform from './Waveform.vue';
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

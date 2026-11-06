<template>
<div>

    <div class="row q-mx-md">
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

        <div class='row col-2 justify-end'>
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
    
        </div>
    </div>

</div>
</template>

<script lang='ts' setup>
import Waveform from './Waveform.vue';
import { PLACEHOLDER_IMG, httpUrl } from '../scripts/utils';
import { computed, onDeactivated, onMounted, ref } from 'vue';
import { get1t } from '../scripts/onetagger';

const $1t = get1t();
const playButton = ref<any>();

onMounted(() => {
    // Unfocus callback (keeps Space from re-triggering the focused play button)
    $1t.quickTagUnfocus = () => {
        playButton.value!.$el.focus();
        playButton.value!.$el.blur();
    }
});

onDeactivated(() => $1t.quickTagUnfocus = () => {});

const art = computed(() => `${httpUrl()}/thumb?path=${encodeURIComponent($1t.player.value.path??'')}`);

</script>
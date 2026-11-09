<template>
<div>
    <div class='row items-center justify-between q-pr-lg'>
        <span class='q-pr-sm monospace q-pb-xs'>{{time}}</span>

        <div 
            ref='waveform' 
            @mouseover='onHover' 
            @mouseleave="hover = false" 
            @click='seek'
            :style='waveformBackground'
            class='waveform' 
        >
            <span v-for='(wave, i) in $1t.player.value.waveform' class='wavefont' :key='wave + i'>
                {{waveChar(wave)}}
            </span>
            <!-- Audition analysis overlays (experimental) -->
            <div v-if='markers' class='wf-overlay' aria-hidden='true'>
                <div v-if='markers.introPct !== undefined' class='wf-section' :style='{ left: 0, width: markers.introPct + "%" }'></div>
                <div v-if='markers.outroPct !== undefined' class='wf-section' :style='{ right: 0, width: (100 - markers.outroPct) + "%" }'></div>
                <div v-if='markers.dropPct !== undefined' class='wf-drop' :style='{ left: markers.dropPct + "%" }'></div>
                <div v-if='markers.dropPct !== undefined' class='wf-drop-label' :style='{ left: "calc(" + markers.dropPct + "% + 4px)" }'>drop</div>
            </div>
        </div>

        <span class='q-pl-sm monospace q-pb-xs'>{{duration($1t.player.value.duration)}}</span>

    </div>
</div>
</template>

<script lang='ts' setup>
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { get1t } from '../scripts/onetagger';
import { getTagStore } from '../scripts/tagstore';

const $1t = get1t();
const tagStore = getTagStore();

/** Section markers as percentages, only for the loaded track with the flag on */
const markers = computed(() => {
    if (!$1t.settings.value.flowtag?.analysisEnabled) return undefined;
    const a = tagStore.trackAnalysis.value;
    const player = $1t.player.value;
    if (!a || a.trackId !== player.trackId || !player.duration) return undefined;
    const pct = (ms?: number) => (ms === undefined ? undefined : Math.min((ms / player.duration) * 100, 100));
    const introPct = pct(a.introMs);
    const outroPct = pct(a.outroMs);
    const dropPct = pct(a.dropMs);
    if (introPct === undefined && outroPct === undefined && dropPct === undefined) return undefined;
    return { introPct, outroPct, dropPct };
});
const hover = ref(false);
const pos = ref(0);
const waveform = ref<any>();

// Generate wave character
function waveChar(wave: number) {
    return String.fromCharCode(0x100 + Math.min(Math.round((wave + 0.05) * 100), 100));
}

function waveOffset(e: MouseEvent) {
    return (e.pageX - waveform.value.offsetLeft) / waveform.value.clientWidth;
}

// Mouse hover fill
function onHover(e: MouseEvent) {
    hover.value = true;
    pos.value = waveOffset(e);
}

// On click seek
function seek(e: MouseEvent) {
    let pos = waveOffset(e) * $1t.player.value.duration;
    $1t.player.value.seek(Math.round(pos));
}

// Pretty print duration
function duration(a: number) {
    let s = Math.round(a / 1000);
    return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, '0')}`
}

// Update player position
let interval: any = undefined;
onMounted(() => {
    interval = setInterval(() => {
        if (hover.value) return;
        pos.value = ($1t.player.value.position / $1t.player.value.duration);
    }, 300);
});
onUnmounted(() => {
    clearInterval(interval);
});

const time = computed(() => duration(pos.value * $1t.player.value.duration));
const waveformBackground = computed(() => {
    if (!hover.value) {
        return `background-image: linear-gradient(to right, var(--wave-played) ${pos.value*100}%, var(--wave-rest) ${pos.value*100}%, var(--wave-rest));`;
    } else {
        // Calculate light and dark parts
        let hoverPos = pos.value * 100.0;
        let audioPos = ($1t.player.value.position / $1t.player.value.duration) * 100.0;
        // Generate gradient
        if (hoverPos > audioPos) {
            return `background-image: linear-gradient(to right, var(--wave-played) ${audioPos}%, var(--wave-hover) ${audioPos}%, var(--wave-hover) ${hoverPos}%, var(--wave-rest) ${hoverPos}%, var(--wave-rest));`;
        } else {
            return `background-image: linear-gradient(to right, var(--wave-played) ${hoverPos}%, var(--wave-hover) ${hoverPos}%, var(--wave-hover) ${audioPos}%, var(--wave-rest) ${audioPos}%, var(--wave-rest));`;
        }
    }

})

</script>

<style>

@keyframes wave-anim {
    0% {
        transform: scale(1, 0.1);        
    }

    100% {
        transform: scale(1, 1.0);
    }
}

.waveform {
    /* background: linear-gradient(to right, var(--wave-played) var(--waveform-filled), var(--wave-rest) var(--waveform-filled), var(--wave-rest)); */
    background-clip: text;
    color: transparent;
    -webkit-background-clip: text;
	-webkit-text-fill-color: transparent;
    cursor: pointer;
    position: relative;
}
.wf-overlay {
    position: absolute;
    inset: 0;
    pointer-events: none;
    -webkit-text-fill-color: initial;
}
.wf-section {
    position: absolute;
    top: 0;
    height: 100%;
    background: repeating-linear-gradient(135deg, rgba(255, 255, 255, 0.05) 0 5px, transparent 5px 10px);
}
.wf-drop {
    position: absolute;
    top: 0;
    width: 2px;
    height: 100%;
    background: var(--ft-warn, #d29922);
}
.wf-drop-label {
    position: absolute;
    top: 2px;
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ft-warn, #d29922);
}

.wavefont {
    font-family: wavefont, blank !important;
    font-variation-settings: 'wdth' var(--waveform-wave), 'algn' 0.5, 'radi' 30;
    font-size: 40px;
    margin: 1px;
    /* display: inline-block; */
    animation-name: wave-anim;
    animation-duration: 200ms;
}

</style>
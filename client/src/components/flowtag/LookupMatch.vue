<template>
<label class='lookup-match flex items-start gap-3 px-3 py-2.5 rounded-md cursor-pointer hover:bg-ft-hover' :class='{ on: selected }'>
    <Checkbox :model-value='selected' class='mt-1' :aria-label='`Use ${match.track.title}`' @update:model-value='emit("toggle")' />
    <img
        :src='artFailed ? PLACEHOLDER_IMG : (match.track.thumbnail ?? match.track.art ?? PLACEHOLDER_IMG)'
        class='size-11 shrink-0 rounded object-cover bg-ft-raised'
        alt=''
        @error='artFailed = true'
    />
    <div class='flex-1 min-w-0'>
        <div class='flex items-center gap-2 text-[10.5px] font-semibold uppercase tracking-[0.06em] text-ft-text-3'>
            <span>{{ match.track.platform }}</span>
            <span class='ft-num' :class='accuracyClass'>{{ (match.accuracy * 100).toFixed(1) }}%</span>
            <span v-if='match.reason !== "fuzzy"'>{{ match.reason }}</span>
        </div>
        <div class='mt-0.5 text-[13px] leading-snug truncate'>
            <span class='text-ft-text-2'>{{ match.track.artists.join(', ') }}</span>
            <span class='text-ft-text-3'> — </span>
            <span class='text-ft-text font-medium'>{{ match.track.title }}</span>
            <span v-if='match.track.version' class='text-ft-text-2'> ({{ match.track.version }})</span>
        </div>
        <div class='mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-ft-text-3'>
            <span v-if='match.track.album' class='truncate max-w-[260px]'>{{ match.track.album }}</span>
            <span v-if='match.track.genres.length'>{{ match.track.genres.join(', ') }}</span>
            <span v-if='match.track.bpm' class='ft-num'>{{ match.track.bpm }} BPM</span>
            <span v-if='match.track.key' class='ft-num font-medium' :style='keyColorStyle(match.track.key)'>{{ match.track.key }}</span>
            <span v-if='match.track.release_date' class='ft-num'>{{ match.track.release_date }}</span>
        </div>
    </div>
    <Hint v-if='match.track.url' text='Open on the platform'>
        <Button size='icon-sm' :aria-label='`Open ${match.track.title} on ${match.track.platform}`' @click.prevent.stop='$1t.url(match.track.url)'>
            <ExternalLinkIcon class='size-3.5' />
        </Button>
    </Hint>
</label>
</template>

<script lang='ts' setup>
import { computed, ref } from 'vue';
import { ExternalLinkIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import Hint from './Hint.vue';
import { get1t } from '../../scripts/onetagger';
import { keyColorStyle } from '../../scripts/musickey';
import { PLACEHOLDER_IMG } from '../../scripts/utils';
import type { TrackMatch } from '../../scripts/manualtag';

/** One online-lookup result: what the platform found and how sure the match is */
const props = defineProps<{ match: TrackMatch; selected: boolean }>();
const emit = defineEmits<{ (e: 'toggle'): void }>();

const $1t = get1t();
const artFailed = ref(false);

const accuracyClass = computed(() => {
    if (props.match.accuracy >= 1) return 'text-ft-playing';
    if (props.match.accuracy > 0.85) return 'text-ft-warn';
    return 'text-ft-danger';
});
</script>

<style lang='scss' scoped>
.lookup-match.on { background: var(--ft-raised); box-shadow: inset 0 0 0 1px var(--ft-line-strong); }
</style>

<template>
<Dialog :open='show' @update:open='(o: boolean) => { if (!o) exit(); }'>
    <DialogContent
        class='flex flex-col gap-0 p-0 sm:max-w-[700px] h-[min(82vh,760px)]'
        :show-close-button='false'
        @interact-outside.prevent
        @escape-key-down='(e: KeyboardEvent) => { if (saving) e.preventDefault(); }'
    >
        <!-- Header -->
        <div class='flex items-center gap-3 px-5 pt-4 pb-3 border-b border-ft-line'>
            <DialogTitle>Online lookup</DialogTitle>
            <DialogDescription class='sr-only'>Search metadata platforms for this track and apply a match</DialogDescription>
            <div class='flex-1'></div>
            <div class='lookup-path ft-mono truncate text-ft-text-3' :title='path'>{{ path }}</div>
        </div>

        <!-- Body -->
        <div class='flex-1 min-h-0 overflow-y-auto px-3 py-3'>
            <!-- Results -->
            <template v-if='mt.busy || mt.done'>
                <div v-if='mt.busy && !mt.done' class='flex items-center gap-2 px-3 pb-2 text-xs text-ft-text-3'>
                    <LoaderCircleIcon class='size-3.5 animate-spin' />
                    Searching {{ $1t.config.value.platforms.length }} platform(s)…
                </div>
                <div v-if='mt.done && mt.matches.length === 0' class='flex flex-col items-center justify-center text-center h-full py-10'>
                    <SearchXIcon class='size-9 text-ft-text-3 mb-3' :stroke-width='1.25' />
                    <div class='text-[15px] font-semibold text-ft-text'>No results</div>
                    <div class='mt-1 text-sm text-ft-text-3 max-w-[340px]'>
                        Enable more platforms, or correct the track's title and artist (E) and search again.
                    </div>
                </div>
                <LookupMatch
                    v-for='(match, i) in mt.matches'
                    :key='i'
                    :match='match'
                    :selected='selected.includes(match)'
                    @toggle='toggleMatch(match)'
                />
            </template>

            <!-- Search settings -->
            <div v-else class='flex flex-col gap-5 px-2'>
                <section>
                    <div class='ft-label mb-2'>Platforms <span class='normal-case tracking-normal font-normal'>· drag to set priority</span></div>
                    <AutotaggerPlatforms />
                </section>
                <section>
                    <div class='ft-label mb-2'>Tags to fetch</div>
                    <AutotaggerTags />
                </section>
                <AutotaggerPlatformSpecific />
            </div>
        </div>

        <!-- Footer -->
        <div class='flex items-center gap-2 px-5 py-3 border-t border-ft-line'>
            <Popover v-if='mt.errors.length > 0'>
                <PopoverTrigger as-child>
                    <Button size='sm' class='text-ft-danger hover:text-ft-danger'>
                        <CircleAlertIcon class='size-3.5' />{{ mt.errors.length }} platform(s) failed
                    </Button>
                </PopoverTrigger>
                <PopoverContent align='start' side='top' class='w-[420px] max-h-[300px] overflow-y-auto'>
                    <div v-for='(error, i) in mt.errors' :key='i' class='text-xs text-ft-text-2 mb-1.5 break-words'>
                        <span class='ft-mono text-ft-text uppercase'>{{ error.platform }}</span>: {{ error.error }}
                    </div>
                </PopoverContent>
            </Popover>
            <div class='flex-1'></div>
            <Button v-if='!saving' @click='exit'>Close</Button>
            <Button v-if='!mt.done' variant='default' :disabled='mt.busy || $1t.config.value.platforms.length === 0' @click='start'>
                <LoaderCircleIcon v-if='mt.busy' class='size-3.5 animate-spin' />
                Search
            </Button>
            <Button v-if='selected.length > 0' variant='default' :disabled='saving' @click='apply'>
                <LoaderCircleIcon v-if='saving' class='size-3.5 animate-spin' />
                Apply {{ selected.length > 1 ? `${selected.length} matches` : 'match' }}
            </Button>
        </div>
    </DialogContent>
</Dialog>
</template>

<script lang='ts' setup>
import { computed, ref, toRefs, watch } from 'vue';
import { CircleAlertIcon, LoaderCircleIcon, SearchXIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { notify } from '@/lib/notify';
import { alertDialog } from '@/lib/dialogs';
import { TrackMatch } from '../scripts/manualtag';
import { get1t } from '../scripts/onetagger';
import { AutotaggerConfig } from '../scripts/autotagger';
import AutotaggerPlatforms from './AutotaggerPlatforms.vue';
import AutotaggerTags from './AutotaggerTags.vue';
import AutotaggerPlatformSpecific from './AutotaggerPlatformSpecific.vue';
import LookupMatch from './flowtag/LookupMatch.vue';

const $1t = get1t();
const show = ref(false);
const emit = defineEmits(['exit']);
const props = defineProps({
    path: { type: String, required: false }
});
const { path } = toRefs(props);
const saving = ref(false);
const selected = ref<TrackMatch[]>([]);
const mt = computed(() => $1t.manualTag.value);
let cachedConfig = {};

/// Start manual tagger
function start() {
    $1t.manualTag.value.reset();

    // Generate config
    let config = JSON.parse(JSON.stringify($1t.config.value));
    config.path = '';
    if ($1t.spotify.value.clientId && $1t.spotify.value.clientSecret) {
        config.spotify = {
            clientId: $1t.spotify.value.clientId,
            clientSecret: $1t.spotify.value.clientSecret,
        }
    }
    cachedConfig = config;

    // Start
    $1t.manualTag.value.tagTrack(path!.value!, config);
}

/// Add or remove match
function toggleMatch(match: TrackMatch) {
    selected.value = selected.value.includes(match)
        ? selected.value.filter((m) => m !== match)
        : [...selected.value, match];
}

/// Exit manual tagger
function exit() {
    if (saving.value) return;
    $1t.manualTag.value.reset();
    selected.value = [];
    show.value = false;
    emit('exit');
}

/// Apply the matches
async function apply() {
    saving.value = true;
    let response: any = await $1t.manualTag.value.apply(selected.value, path!.value!, cachedConfig as AutotaggerConfig);
    saving.value = false;
    if (response.status == 'ok') {
        notify({ message: 'Track saved', timeout: 3000 });
    } else {
        await alertDialog({ title: 'Failed to save track', message: String(response.error) });
    }
    exit();
}

// Show / Hide
watch(path!, () => {
    show.value = !!(path!.value);
});
</script>

<style lang='scss' scoped>
.lookup-path { max-width: 60%; direction: rtl; text-align: right; }
</style>

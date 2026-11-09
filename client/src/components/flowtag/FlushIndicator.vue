<template>
<div class='flex items-center'>
    <!-- Pending writes -->
    <span v-if='pending > 0' class='inline-flex items-center gap-1 text-xs mr-2 text-ft-warn ft-num' title='Tag changes not yet written to files'>
        <RotateCwIcon class='size-3' /> {{ pending }} pending
    </span>
    <!-- Write errors -->
    <Popover v-if='store.flush.value.errors.length > 0'>
        <PopoverTrigger as-child>
            <Button size='sm' class='text-ft-danger hover:text-ft-danger'>
                <CircleAlertIcon class='size-3.5' />{{ store.flush.value.errors.length }} failed
            </Button>
        </PopoverTrigger>
        <PopoverContent align='end' class='w-auto max-w-[420px] p-4'>
            <div class='text-sm font-semibold text-ft-text mb-2'>Files that could not be written</div>
            <div v-for='err in store.flush.value.errors.slice(0, 10)' :key='err.trackId' class='text-xs text-ft-text-2 mb-1 break-words'>
                <b class='text-ft-text'>{{ trackName(err.trackId) }}</b>: {{ err.error }}
            </div>
            <Button variant='default' size='sm' class='mt-2' @click='retry'><RefreshCwIcon class='size-3.5' />Retry all</Button>
        </PopoverContent>
    </Popover>
</div>
</template>

<script lang='ts' setup>
import { computed } from 'vue';
import { CircleAlertIcon, RefreshCwIcon, RotateCwIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { get1t } from '../../scripts/onetagger';
import { getTagStore } from '../../scripts/tagstore';

const store = getTagStore();

// Pending is derived from track state so it updates live with every tag edit
const pending = computed(() => store.tracks.value.reduce((n, t) => n + (t.needsFlush ? 1 : 0), 0));

function trackName(trackId: number): string {
    return store.tracks.value.find((t) => t.id === trackId)?.title ?? `#${trackId}`;
}

function retry() {
    get1t().send('flushRetry');
}
</script>

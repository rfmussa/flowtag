<template>
<div class='row items-center no-wrap'>
    <!-- Pending writes -->
    <span v-if='pending > 0' class='text-caption text-orange q-mr-sm' title='Tag changes not yet written to files'>
        <q-icon name='mdi-sync' /> {{ pending }} pending
    </span>
    <!-- Write errors -->
    <q-btn
        v-if='store.flush.value.errors.length > 0'
        flat dense no-caps
        color='negative'
        icon='mdi-alert-circle-outline'
        :label='`${store.flush.value.errors.length} failed`'
    >
        <q-menu anchor='bottom right' self='top right'>
            <div class='q-pa-md' style='max-width: 420px'>
                <div class='text-subtitle2 q-mb-sm'>Files that could not be written</div>
                <div v-for='err in store.flush.value.errors.slice(0, 10)' :key='err.trackId' class='text-caption q-mb-xs'>
                    <b>{{ trackName(err.trackId) }}</b>: {{ err.error }}
                </div>
                <q-btn
                    dense no-caps color='primary' label='Retry all' icon='mdi-refresh'
                    class='q-mt-sm' @click='retry'
                />
            </div>
        </q-menu>
    </q-btn>
</div>
</template>

<script lang='ts' setup>
import { computed } from 'vue';
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

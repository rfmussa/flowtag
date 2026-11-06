<template>
<div v-if='store.scan.value.running' class='row items-center no-wrap text-caption text-grey-5'>
    <q-linear-progress
        :indeterminate='scan.total === 0'
        :value='scan.total ? scan.done / scan.total : 0'
        color='primary'
        track-color='grey-9'
        style='width: 120px'
        class='q-mr-sm'
    ></q-linear-progress>
    <span>Scanning… {{ scan.done }}/{{ scan.total }}</span>
</div>
<div v-else-if='scan.result' class='row items-center no-wrap text-caption text-grey-5'>
    <span>
        Scan: {{ scan.result.added }} added, {{ scan.result.updated }} updated,
        {{ scan.result.removed }} missing, {{ scan.result.failed }} failed
    </span>
    <q-btn flat round dense size='xs' icon='mdi-close' class='q-ml-xs' @click='dismiss'>
        <q-tooltip>Dismiss</q-tooltip>
    </q-btn>
</div>
</template>

<script lang='ts' setup>
import { computed } from 'vue';
import { getTagStore } from '../../scripts/tagstore';

const store = getTagStore();
const scan = computed(() => store.scan.value);

function dismiss() {
    store.scan.value = { running: false, done: 0, total: 0 };
}
</script>

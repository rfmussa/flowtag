<template>
<div v-if='store.scan.value.running' class='flex items-center gap-2 text-xs text-ft-text-3 whitespace-nowrap'>
    <Progress :model-value='scan.total ? (scan.done / scan.total) * 100 : 0' class='w-[120px]' />
    <span class='tabular-nums'>Scanning… {{ scan.done }}/{{ scan.total }}</span>
</div>
<div v-else-if='scan.result' class='flex items-center gap-1 text-xs text-ft-text-3 whitespace-nowrap'>
    <span class='tabular-nums'>
        Scan: {{ scan.result.added }} added, {{ scan.result.updated }} updated,
        {{ scan.result.removed }} missing, {{ scan.result.failed }} failed
    </span>
    <Hint text='Dismiss'>
        <Button size='icon-xs' aria-label='Dismiss scan result' @click='dismiss'><XIcon /></Button>
    </Hint>
</div>
</template>

<script lang='ts' setup>
import { XIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { Progress } from '@/components/ui/progress';
import Hint from './Hint.vue';
import { computed } from 'vue';
import { getTagStore } from '../../scripts/tagstore';

const store = getTagStore();
const scan = computed(() => store.scan.value);

function dismiss() {
    store.scan.value = { running: false, done: 0, total: 0 };
}
</script>

<template>
<!-- Writing comments into real files is the costliest operation here: show it -->
<div v-if='exp.running' class='flex items-center gap-2 text-xs text-ft-text-3 whitespace-nowrap' role='status'>
    <Progress :model-value='exp.total ? (exp.done / exp.total) * 100 : 0' class='w-[120px]' />
    <span class='tabular-nums'>Exporting… {{ exp.done.toLocaleString() }}/{{ exp.total.toLocaleString() }}</span>
</div>
<!-- Failures persist like flush errors do; success stays a toast -->
<div v-else-if='exp.result' class='flex items-center gap-1 text-xs whitespace-nowrap' role='status'>
    <span class='tabular-nums text-ft-danger'>Export: {{ exp.result.failed }} failed</span>
    <span class='tabular-nums text-ft-text-3'>· {{ exp.result.written.toLocaleString() }} written</span>
    <Hint text='Dismiss'>
        <Button size='icon-xs' aria-label='Dismiss export result' @click='dismiss'><XIcon /></Button>
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
const exp = computed(() => store.exportStatus.value);

function dismiss() {
    store.exportStatus.value = { running: false, done: 0, total: 0 };
}
</script>

<template>
<!-- Writing comments into real files is the costliest operation here: show it -->
<div v-if='exp.running' class='flex items-center gap-2 text-xs text-ft-text-3 whitespace-nowrap' role='status'>
    <Progress :model-value='exp.total ? (exp.done / exp.total) * 100 : 0' class='w-[120px]' />
    <span class='tabular-nums'>Exporting… {{ exp.done.toLocaleString() }}/{{ exp.total.toLocaleString() }}</span>
</div>
</template>

<script lang='ts' setup>
import { Progress } from '@/components/ui/progress';
import { computed } from 'vue';
import { getTagStore } from '../../scripts/tagstore';

const store = getTagStore();
const exp = computed(() => store.exportStatus.value);
</script>

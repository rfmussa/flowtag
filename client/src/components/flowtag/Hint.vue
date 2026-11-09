<template>
<Tooltip>
    <TooltipTrigger as-child>
        <slot />
    </TooltipTrigger>
    <TooltipContent :side='side'>{{ text }}</TooltipContent>
</Tooltip>
</template>

<script lang='ts' setup>
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';

/** A short hover hint on the wrapped element (shadcn Tooltip, FlowTag styling) */
withDefaults(defineProps<{ text: string; side?: 'top' | 'right' | 'bottom' | 'left' }>(), { side: 'top' });
</script>

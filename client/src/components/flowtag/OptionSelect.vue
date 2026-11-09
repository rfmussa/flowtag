<template>
<Select :model-value='modelValue' @update:model-value='onPick'>
    <SelectTrigger :size='size' :class='triggerClass' :aria-label='label' @click.stop>
        <SelectValue :placeholder='placeholder' />
    </SelectTrigger>
    <SelectContent position='popper' align='start'>
        <SelectItem v-for='o in options' :key='String(o.value)' :value='o.value'>{{ o.label }}</SelectItem>
    </SelectContent>
</Select>
</template>

<script lang='ts' setup generic='T extends string | number'>
import type { HTMLAttributes } from 'vue';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';

/** A labelled option list on shadcn Select: the shape every FlowTag dropdown uses */
withDefaults(defineProps<{
    modelValue: T | undefined;
    options: ReadonlyArray<{ label: string; value: T }>;
    label?: string;
    placeholder?: string;
    size?: 'sm' | 'default';
    triggerClass?: HTMLAttributes['class'];
}>(), { size: 'default' });

const emit = defineEmits<{ (e: 'update:modelValue', v: T): void }>();

function onPick(v: unknown) {
    if (v !== null && v !== undefined) emit('update:modelValue', v as T);
}
</script>

<template>
<Select :model-value='[...modelValue]' multiple @update:model-value='onPick'>
    <SelectTrigger :class='cn("data-[size=default]:h-auto min-h-7 py-1 justify-start", triggerClass)' :aria-label='label'>
        <span v-if='selected.length === 0' class='flex-1 text-left text-ft-text-3'>{{ placeholder }}</span>
        <span v-else class='flex flex-1 flex-wrap gap-1 min-w-0'>
            <span
                v-for='o in selected'
                :key='String(o.value)'
                class='inline-flex items-center gap-1 h-5 max-w-[160px] px-1.5 rounded bg-ft-raised border border-ft-line text-xs text-ft-text'
            >
                <span v-if='o.color' class='size-1.5 shrink-0 rounded-full' :style='{ background: o.color }'></span>
                <span class='truncate'>{{ o.label }}</span>
            </span>
        </span>
    </SelectTrigger>
    <SelectContent position='popper' align='start'>
        <SelectItem v-for='o in options' :key='String(o.value)' :value='o.value'>
            <span v-if='o.color' class='size-2 shrink-0 rounded-full' :style='{ background: o.color }'></span>
            {{ o.label }}
        </SelectItem>
    </SelectContent>
</Select>
</template>

<script lang='ts' setup generic='T extends string | number'>
import { computed, type HTMLAttributes } from 'vue';
import { cn } from '@/lib/utils';
import { Select, SelectContent, SelectItem, SelectTrigger } from '@/components/ui/select';

/** Pick several options; the trigger shows them as chips (optionally with a color dot) */
const props = withDefaults(defineProps<{
    modelValue: readonly T[];
    options: ReadonlyArray<{ label: string; value: T; color?: string }>;
    label?: string;
    placeholder?: string;
    triggerClass?: HTMLAttributes['class'];
}>(), { placeholder: 'Any' });

const emit = defineEmits<{ (e: 'update:modelValue', v: T[]): void }>();

const selected = computed(() => props.options.filter((o) => props.modelValue.includes(o.value)));

function onPick(v: unknown) {
    emit('update:modelValue', Array.isArray(v) ? [...(v as T[])] : []);
}
</script>

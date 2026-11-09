<template>
<Popover v-model:open='open'>
    <!-- No Tooltip here: a Tooltip trigger wrapping this trigger steals the popover's anchor -->
    <PopoverTrigger as-child>
        <button
            type='button'
            class='swatch-btn size-[18px] shrink-0 rounded-full cursor-pointer ring-1 ring-inset ring-white/15 transition-[box-shadow] duration-[80ms] hover:ring-white/40'
            :style='{ backgroundColor: modelValue }'
            :aria-label='label'
            :title='label'
        ></button>
    </PopoverTrigger>
    <PopoverContent align='start' class='w-[188px] p-2.5'>
        <div class='grid grid-cols-6 gap-1.5' role='listbox' :aria-label='label'>
            <button
                v-for='c in TAG_COLORS'
                :key='c'
                type='button'
                role='option'
                :aria-selected='c === current'
                :aria-label='c'
                class='grid size-6 place-items-center rounded-full cursor-pointer ring-1 ring-inset ring-white/10 hover:ring-white/45 focus-visible:outline-2 focus-visible:outline-ft-focus'
                :style='{ backgroundColor: c }'
                @click='pick(c)'
            >
                <CheckIcon v-if='c === current' class='size-3.5 text-white drop-shadow' :stroke-width='3' />
            </button>
        </div>
        <form class='mt-2.5 flex items-center gap-1.5' @submit.prevent='commitHex'>
            <span class='size-5 shrink-0 rounded-md ring-1 ring-inset ring-white/15' :style='{ backgroundColor: hexPreview ?? modelValue }'></span>
            <Input
                v-model='hex'
                class='h-6 px-2 font-mono text-xs'
                :aria-invalid='hex !== "" && !hexPreview'
                aria-label='Hex color'
                spellcheck='false'
                maxlength='7'
                @blur='commitHex'
            />
        </form>
    </PopoverContent>
</Popover>
</template>

<script lang='ts' setup>
import { computed, ref, watch } from 'vue';
import { CheckIcon } from '@lucide/vue';
import { Input } from '@/components/ui/input';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { TAG_COLORS, parseHexColor } from '@/lib/color';

/** Swatch button opening the 12-color tag palette plus a hex field */
const props = withDefaults(defineProps<{ modelValue: string; label?: string }>(), { label: 'Change color' });
const emit = defineEmits<{ (e: 'update:modelValue', v: string): void }>();

const open = ref(false);
const hex = ref(props.modelValue);
const current = computed(() => parseHexColor(props.modelValue));
const hexPreview = computed(() => parseHexColor(hex.value));

watch(() => props.modelValue, (v) => { hex.value = v; });
watch(open, (o) => { if (o) hex.value = props.modelValue; });

function pick(color: string) {
    if (color !== current.value) emit('update:modelValue', color);
    open.value = false;
}

function commitHex() {
    const c = hexPreview.value;
    if (c && c !== current.value) emit('update:modelValue', c);
}
</script>

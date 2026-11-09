<template>
<label class='flex flex-col gap-1 min-w-0'>
    <span class='text-[10.5px] font-semibold uppercase tracking-[0.06em] text-ft-text-3'>{{ LABELS[format] }}</span>
    <Input
        :model-value='value'
        :list='listId'
        spellcheck='false'
        class='font-mono text-xs'
        :aria-invalid='!!error'
        @update:model-value='onInput(String($event))'
    />
    <datalist :id='listId'>
        <option v-for='o in OPTIONS[format]' :key='o' :value='o' />
    </datalist>
    <span v-if='error' class='text-[11px] text-ft-danger'>{{ error }}</span>
    <span v-else-if='custom' class='text-[11px] text-ft-warn'>Custom field (written as TXXX / freeform)</span>
</label>
</template>

<script lang='ts' setup>
import { computed, ref, useId, watch } from 'vue';
import { Input } from '@/components/ui/input';
import { FrameFormat, stripFrameHelper, validateFrameName } from '@/lib/tagframe';
import { MP4, VORBIS, ID3 } from '../scripts/tags';

/** One container format's frame name, with suggestions and safety checks */
const props = withDefaults(defineProps<{ format: FrameFormat; modelValue?: string; tageditor?: boolean }>(), {
    modelValue: '',
    tageditor: false,
});
const emit = defineEmits<{ (e: 'update:modelValue', v: string): void }>();

const LABELS: Record<FrameFormat, string> = { id3: 'ID3 · MP3 AIFF WAV', vorbis: 'Vorbis · FLAC OGG OPUS', mp4: 'MP4 · M4A' };
const OPTIONS: Record<FrameFormat, readonly string[]> = { id3: ID3, vorbis: VORBIS, mp4: MP4 };

const listId = useId();
const value = ref(props.modelValue);
const error = computed(() => validateFrameName(props.format, value.value, props.tageditor));
const custom = computed(() => (props.format === 'id3' || props.format === 'mp4') && value.value.trim().length !== 4);

watch(() => props.modelValue, (v) => { value.value = v ?? ''; });

function onInput(raw: string) {
    const v = stripFrameHelper(props.format, raw.trim());
    value.value = v;
    emit('update:modelValue', v);
}
</script>

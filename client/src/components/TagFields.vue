<template>
<div class='grid grid-cols-3 gap-3'>
    <TagField
        v-for='format in FORMATS'
        :key='format'
        :format='format'
        :tageditor='tageditor'
        :model-value='modelValue[format]'
        @update:model-value='(v: string) => update(format, v)'
    />
</div>
</template>

<script lang='ts' setup>
import { FrameName } from '../scripts/utils';
import type { FrameFormat } from '@/lib/tagframe';
import TagField from './TagField.vue';

/** A tag's frame name in each container format (ID3, Vorbis, MP4) */
const props = withDefaults(defineProps<{ modelValue: FrameName; tageditor?: boolean }>(), { tageditor: false });
const emit = defineEmits<{ (e: 'update:modelValue', v: FrameName): void }>();

const FORMATS: readonly FrameFormat[] = ['id3', 'vorbis', 'mp4'];

function update(format: FrameFormat, value: string) {
    const next = new FrameName(props.modelValue.id3, props.modelValue.vorbis, props.modelValue.mp4);
    next[format] = value;
    emit('update:modelValue', next);
}
</script>

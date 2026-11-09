<template>
<div>
    <div class='grid grid-cols-3 gap-x-4 gap-y-0.5'>
        <label
            v-for='tag in SUPPORTED_TAGS'
            :key='tag.tag'
            class='flex items-center gap-2 h-7 text-sm cursor-pointer'
            :class='isSupported(tag.tag) ? "text-ft-text-2 hover:text-ft-text" : "text-ft-text-3 opacity-50 cursor-not-allowed"'
            :title='isSupported(tag.tag) ? tag.tooltip : "None of the selected platforms provide this tag"'
        >
            <Checkbox
                :disabled='!isSupported(tag.tag)'
                :model-value='$1t.config.value.tags.includes(tag.tag)'
                @update:model-value='toggleTag(tag.tag)'
            />
            <span class='truncate'>{{ tag.label }}</span>
        </label>
    </div>
    <div class='flex gap-1 mt-2 -ml-2'>
        <Button size='sm' @click='setTags("enableAll")'>Enable all</Button>
        <Button size='sm' @click='setTags("disableAll")'>Disable all</Button>
        <Button size='sm' @click='setTags("toggle")'>Invert</Button>
    </div>
</div>
</template>

<script lang='ts' setup>
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { get1t } from '../scripts/onetagger';
import { SUPPORTED_TAGS, SupportedTag } from '../scripts/autotagger';

/** Which tags the lookup writes when a match is applied */
const $1t = get1t();

/// Check if tag is supported on selected platforms
function isSupported(tag: SupportedTag) {
    if (tag == SupportedTag.MetaTags) return true;
    return ($1t.info.value.platforms.find(p => $1t.config.value.platforms.includes(p.platform.id) && p.supportedTags.includes(tag))) != null;
}

function toggleTag(tag: SupportedTag) {
    const tags = $1t.config.value.tags;
    $1t.config.value.tags = tags.includes(tag) ? tags.filter((t) => t !== tag) : [...tags, tag];
}

function setTags(mode: 'enableAll' | 'disableAll' | 'toggle') {
    const all = SUPPORTED_TAGS.map((t) => t.tag);
    switch (mode) {
        case 'enableAll':
            $1t.config.value.tags = all;
            break;
        case 'disableAll':
            $1t.config.value.tags = [];
            break;
        case 'toggle':
            $1t.config.value.tags = all.filter((t) => !$1t.config.value.tags.includes(t));
            break;
    }
}
</script>

<template>
<draggable
    v-model='$1t.info.value.platforms'
    item-key='id'
    handle='.drag-handle'
    class='grid grid-cols-2 gap-1.5'
    @update='syncPlatforms'
>
    <template #item='{ element: platform }'>
        <label
            class='platform-row flex items-center gap-2.5 h-11 pl-1.5 pr-3 rounded-md border border-ft-line bg-ft-panel cursor-pointer hover:border-ft-line-strong'
            :class='{ on: isEnabled(platform.platform.id) }'
        >
            <GripVerticalIcon class='drag-handle size-4 shrink-0 text-ft-text-3 cursor-grab' aria-hidden='true' />
            <Checkbox
                :model-value='isEnabled(platform.platform.id)'
                :aria-label='`Search ${platform.platform.name}`'
                @update:model-value='toggle(platform.platform.id)'
            />
            <img :src='platform.icon' class='size-6 shrink-0 object-contain' alt='' />
            <span class='flex-1 min-w-0 truncate text-sm font-medium text-ft-text'>{{ platform.platform.name }}</span>
            <span class='flex items-center gap-1.5 text-ft-text-3'>
                <Hint v-if='platform.platform.requiresAuth' text='Requires an account'>
                    <LockIcon class='size-3' />
                </Hint>
                <Hint v-if='hasLyrics(platform)' text='Can fetch lyrics'>
                    <MicVocalIcon class='size-3' />
                </Hint>
                <Hint :text='speedText(platform)'>
                    <GaugeIcon class='size-3' :class='{ "opacity-50": platform.platform.maxThreads === 1 }' />
                </Hint>
            </span>
        </label>
    </template>
</draggable>
</template>

<script lang='ts' setup>
import { onMounted } from 'vue';
import draggable from 'vuedraggable';
import { GaugeIcon, GripVerticalIcon, LockIcon, MicVocalIcon } from '@lucide/vue';
import { Checkbox } from '@/components/ui/checkbox';
import Hint from './flowtag/Hint.vue';
import { get1t } from '../scripts/onetagger.js';
import { AutotaggerPlatform, SupportedTag } from '../scripts/autotagger';

/** Which platforms the lookup searches, in priority order (drag to reorder) */
const $1t = get1t();

function toggle(platform: string) {
    const enabled = $1t.config.value.platforms;
    $1t.config.value.platforms = enabled.includes(platform)
        ? enabled.filter((p) => p !== platform)
        : [...enabled, platform];
}

function isEnabled(platform: string) {
    return $1t.config.value.platforms.includes(platform);
}

// Sync platforms order to config
function syncPlatforms() {
    $1t.config.value.platforms = $1t.info.value.platforms.map((p) => p.platform.id).filter((p) => $1t.config.value.platforms.includes(p));
}

function hasLyrics(platform: AutotaggerPlatform) {
    return platform.supportedTags.includes(SupportedTag.UnsyncedLyrics) || platform.supportedTags.includes(SupportedTag.SyncedLyrics);
}

function speedText(platform: AutotaggerPlatform) {
    const n = platform.platform.maxThreads;
    return n ? `Up to ${n} concurrent search${n === 1 ? '' : 'es'}` : 'Unlimited concurrent searches';
}

onMounted(() => {
    $1t.info.value.platforms.sort((a, b) => {
        let x = $1t.config.value.platforms.indexOf(a.platform.id);
        let y = $1t.config.value.platforms.indexOf(b.platform.id);
        if (x == -1) x = 1000;
        if (y == -1) y = 1000;
        return x - y;
    });
});
</script>

<style lang='scss' scoped>
.platform-row { user-select: none; }
.platform-row.on { background: var(--ft-raised); border-color: var(--ft-line-strong); }
</style>

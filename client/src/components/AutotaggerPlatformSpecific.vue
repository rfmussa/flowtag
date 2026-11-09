<template>
<div class='flex flex-col gap-5'>
    <section v-for='platform in platforms' :key='platform.id'>
        <div class='ft-label mb-2'>{{ platform.name }} options</div>
        <div class='flex flex-col gap-3'>
            <div v-for='option in platform.customOptions.options' :key='option.id'>

                <!-- Number: slider with its value -->
                <div v-if='option.value.type == "number"' class='flex items-center gap-3'>
                    <span class='w-[180px] shrink-0 text-sm text-ft-text-2' :title='option.tooltip'>{{ option.label }}</span>
                    <Slider
                        :model-value='[custom(platform.id)[option.id]]'
                        :min='option.value.min'
                        :max='option.value.max'
                        :step='option.value.step'
                        class='flex-1'
                        :aria-label='option.label'
                        @update:model-value='(v) => setOption(platform.id, option.id, v?.[0])'
                    />
                    <span class='w-12 text-right ft-num text-sm text-ft-text'>{{ custom(platform.id)[option.id] }}</span>
                </div>

                <!-- Custom tag frame per format -->
                <div v-else-if='option.value.type == "tag"'>
                    <div class='text-sm text-ft-text-2 mb-1.5'>{{ option.label }}</div>
                    <TagFields v-model='custom(platform.id)[option.id]' />
                </div>

                <!-- Pick one value -->
                <div v-else-if='option.value.type == "option"' class='flex items-center gap-3'>
                    <span class='w-[180px] shrink-0 text-sm text-ft-text-2' :title='option.tooltip'>{{ option.label }}</span>
                    <OptionSelect
                        :model-value='custom(platform.id)[option.id]'
                        :options='option.value.values.map((v: string) => ({ label: v, value: v }))'
                        :label='option.label'
                        @update:model-value='(v: string) => setOption(platform.id, option.id, v)'
                    />
                </div>

                <!-- Text (or secret) -->
                <div v-else-if='option.value.type == "string"'>
                    <label class='flex items-center gap-3'>
                        <span class='w-[180px] shrink-0 text-sm text-ft-text-2'>{{ option.label }}</span>
                        <Input
                            v-model='custom(platform.id)[option.id]'
                            :type='option.value.hidden ? "password" : "text"'
                            spellcheck='false'
                            class='flex-1'
                        />
                    </label>
                    <div v-if='option.tooltip' class='mt-1 ml-[192px] text-xs text-ft-text-3 option-help' v-html='option.tooltip'></div>
                </div>

                <!-- On / off -->
                <label v-else-if='option.value.type == "boolean"' class='flex items-center justify-between gap-3 h-7 cursor-pointer'>
                    <span class='text-sm text-ft-text-2' :title='option.tooltip'>{{ option.label }}</span>
                    <Switch v-model='custom(platform.id)[option.id]' />
                </label>

                <!-- Action (sends the platform's config callback) -->
                <div v-else-if='option.value.type == "button"' class='flex items-center gap-3'>
                    <Button variant='default' size='sm' @click='callback(platform.id, option.id)'>{{ option.label }}</Button>
                    <span v-if='option.tooltip' class='text-xs text-ft-text-3 option-help' v-html='option.tooltip'></span>
                </div>
            </div>
        </div>
    </section>

    <!-- Spotify needs the user's own developer app -->
    <section v-if='spotify'>
        <div class='ft-label mb-2'>Spotify</div>
        <div v-if='$1t.spotify.value.authorized' class='text-sm text-ft-text-2'>Signed in to Spotify.</div>
        <SpotifyLogin v-else />
    </section>
</div>
</template>

<script lang='ts' setup>
import { computed } from 'vue';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Slider } from '@/components/ui/slider';
import { Switch } from '@/components/ui/switch';
import OptionSelect from './flowtag/OptionSelect.vue';
import TagFields from './TagFields.vue';
import SpotifyLogin from './SpotifyLogin.vue';
import { get1t } from '../scripts/onetagger';

/** Settings a platform declares for itself (sliders, selects, tag frames...) */
const $1t = get1t();
const spotify = computed(() => $1t.config.value.platforms.includes('spotify'));
const platforms = computed(() => $1t.info.value.platforms
    .filter((p) => $1t.config.value.platforms.includes(p.platform.id) && p.platform.customOptions.options.length > 0)
    .map((p) => p.platform));

function custom(platformId: string): Record<string, any> {
    return $1t.config.value.custom[platformId];
}

function setOption(platformId: string, optionId: string, value: unknown) {
    if (value === undefined) return;
    $1t.config.value.custom[platformId][optionId] = value;
}

function callback(platformId: string, optionId: string) {
    $1t.send('configCallback', { platform: platformId, config: $1t.config.value.custom[platformId], id: optionId });
}
</script>

<style lang='scss' scoped>
.option-help :deep(a) { color: var(--ft-text-2); text-decoration: underline; }
</style>

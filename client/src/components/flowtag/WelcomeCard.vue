<template>
<div class='welcome-card' role='status'>
    <HistoryIcon class='welcome-icon size-4' :stroke-width='1.5' />
    <div class='welcome-lines'>
        <div class='welcome-title'>Welcome back</div>
        <div v-for='line in lines' :key='line' class='welcome-line'>{{ line }}</div>
    </div>
    <Button size='sm' variant='ghost' tabindex='-1' @click='emit("dismiss")'>Dismiss</Button>
    <Button size='sm' tabindex='-1' @click='emit("continue")'>Continue</Button>
</div>
</template>

<script lang='ts' setup>
import { computed } from 'vue';
import { HistoryIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';

const props = defineProps<{ text: string }>();
const emit = defineEmits<{ (e: 'dismiss'): void; (e: 'continue'): void }>();

const lines = computed(() => props.text.split('\n'));
</script>

<style lang='scss' scoped>
.welcome-card {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 8px 12px 0;
    padding: 8px 12px;
    background: var(--ft-panel);
    border: 1px solid var(--ft-line);
    border-radius: 8px;
    flex-shrink: 0;
}
.welcome-icon { color: var(--ft-text-3); flex-shrink: 0; }
.welcome-lines { flex: 1; min-width: 0; font-size: 12px; }
.welcome-title { color: var(--ft-text); font-weight: 500; }
.welcome-line { color: var(--ft-text-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>

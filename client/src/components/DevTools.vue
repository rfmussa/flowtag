<template>
<Dialog v-model:open='open'>
    <DialogContent class='flex flex-col gap-3 sm:max-w-[900px] h-[min(86vh,820px)]'>
        <DialogTitle>Dev tools</DialogTitle>
        <DialogDescription class='sr-only'>Build information, debugging actions and the backend log</DialogDescription>

        <dl class='grid grid-cols-[max-content_1fr] gap-x-4 gap-y-0.5 text-xs'>
            <dt class='text-ft-text-3'>Version</dt><dd class='ft-mono text-ft-text select-text'>{{ $1t.info.value.version }} ({{ $1t.info.value.commit }})</dd>
            <dt class='text-ft-text-3'>OS</dt><dd class='ft-mono text-ft-text'>{{ $1t.info.value.os }}</dd>
            <dt class='text-ft-text-3'>Custom platforms</dt><dd class='ft-mono text-ft-text'>{{ $1t.info.value.platforms.filter(p => !p.builtIn).map(p => p.platform.id).join(', ') || 'none' }} · compat {{ $1t.info.value.customPlatformCompat }}</dd>
            <dt class='text-ft-text-3'>Data directory</dt><dd class='ft-mono text-ft-text select-text truncate'>{{ $1t.info.value.dataDir }}</dd>
            <dt class='text-ft-text-3'>Working directory</dt><dd class='ft-mono text-ft-text select-text truncate'>{{ $1t.info.value.workDir }}</dd>
            <dt class='text-ft-text-3'>Start context</dt><dd class='ft-mono text-ft-text'>{{ $1t.info.value.startContext }}</dd>
        </dl>

        <div class='flex flex-wrap items-center gap-1 -ml-2'>
            <Button size='sm' @click='reload()'>Reload</Button>
            <Button size='sm' @click='devServer()'>Go to dev server</Button>
            <Button size='sm' @click='devtools()'>Open webview devtools</Button>
            <Button size='sm' @click='$1t.send("openSettingsFolder")'>Open data folder</Button>
            <label class='ml-auto flex items-center gap-2 text-xs text-ft-text-2 cursor-pointer'>
                Webview devtools on start (restart needed)
                <Switch :model-value='$1t.settings.value.devtools' @update:model-value='setDevtools' />
            </label>
        </div>

        <div class='flex items-center gap-3 text-xs'>
            <span class='ft-label'>Log</span>
            <label v-for='level in levels' :key='level' class='flex items-center gap-1.5 text-ft-text-2 cursor-pointer'>
                <Checkbox :model-value='filterLevels.includes(level)' @update:model-value='filterLevel(level)' />{{ level }}
            </label>
            <div class='flex-1'></div>
            <Button size='xs' @click='fullLog = !fullLog'>{{ fullLog ? 'This session only' : 'Show full log' }}</Button>
            <Button size='xs' @click='logBottom()'>To bottom</Button>
        </div>
        <div ref='logRef' class='flex-1 min-h-0 overflow-y-auto rounded-md border border-ft-line bg-ft-bg px-3 py-2 select-text'>
            <div v-for='(item, index) in shownLog' :key='index' class='mb-2 ft-mono text-[11px] leading-snug whitespace-pre-wrap break-words'>
                <span class='text-ft-text-3'>{{ item.time }}</span>
                <span class='ml-2 font-semibold' :class='levelClass(item.level)'>{{ item.level }}</span>
                <span class='ml-2 text-ft-text-2'>{{ item.module }}</span>
                <span v-if='item.count > 1' class='ml-2 text-ft-text-3'>×{{ item.count }}</span>
                <div class='text-ft-text'>{{ item.line }}</div>
            </div>
        </div>
    </DialogContent>
</Dialog>
</template>

<script lang='ts' setup>
import { computed, nextTick, ref, watch } from 'vue';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog';
import { Switch } from '@/components/ui/switch';
import { get1t } from '../scripts/onetagger';

/** F3: build info, quick actions and the backend log */
const open = defineModel<boolean>({ default: false });
const $1t = get1t();
const logRef = ref<HTMLElement>();
const fullLog = ref(false);
/** Rendering every line of a long log is slow; the tail is what matters */
const MAX_LOG_ENTRIES = 1500;
const filterLevels = ref<string[]>(['DEBUG', 'WARN', 'ERROR', 'INFO']);

const levels = ['DEBUG', 'WARN', 'ERROR', 'INFO'];

// Add level to filter
function filterLevel(level: string) {
    filterLevels.value = filterLevels.value.includes(level)
        ? filterLevels.value.filter((l) => l !== level)
        : [...filterLevels.value, level];
}

function reload() {
    location.reload();
}

function devServer() {
    window.location.href = 'http://localhost:8080';
}

function devtools() {
    //@ts-ignore
    window.ipc.postMessage('devtools');
}

// Scroll to bottom
function logBottom() {
    logRef.value?.scrollTo({ top: logRef.value.scrollHeight });
}

// Get color for level
function levelClass(level: string) {
    return {
        'INFO': 'text-ft-playing',
        'WARN': 'text-ft-warn',
        'ERROR': 'text-ft-danger',
        'DEBUG': 'text-ft-text-3'
    }[level];
}

function setDevtools(v: boolean) {
    $1t.settings.value.devtools = v;
    $1t.saveSettings(true);
}

const shownLog = computed(() => log.value.slice(-MAX_LOG_ENTRIES));

// Parse the log
const log = computed(() => {
    let lines: { time: string, level: string, module: string, line: string, count: number }[] = [];
    let prev = '';
    ($1t.info.value.log?.split('\n')??[]).forEach(l => {
        if (l.match(/^\d{4}-\d\d-\d\d \d\d:\d\d:\d\d \[INFO|WARN|ERROR|\] /)) {
            let parts = l.split(' ');
            let content = parts.slice(4).join(" ");
            // Deduplicate
            if (content == prev && prev != '') {
                lines[lines.length - 1].count += 1;
                return;
            }

            // Filter
            if (lines.length > 0 && !filterLevels.value.includes(lines[lines.length - 1].level)) {
                lines.pop();
            }

            prev = content;
            lines.push({
                time: `${parts[0]} ${parts[1]}`,
                level: parts[2].substring(1, parts[2].length - 1),
                module: parts[3].substring(0, parts[3].length - 1),
                line: content,
                count: 1,
            });
        } else {
            // Only this log
            if (!fullLog.value && l.includes('Starting OneTagger') && lines.length > 1) {
                lines = lines.slice(lines.length - 1);
            }
            lines[lines.length - 1].line += '\n' + l;
        }
        
    });
    // Filter
    if (lines.length > 0 && !filterLevels.value.includes(lines[lines.length - 1].level)) {
        lines.pop();
    }
    return lines;
});

watch(open, (o) => {
    if (!o) return;
    $1t.send('getLog');
    nextTick(logBottom);
});
watch(() => $1t.info.value.log, () => { if (open.value) nextTick(logBottom); });

</script>

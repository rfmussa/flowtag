<template>
<Dialog :open='open' @update:open='onOpenChange'>
    <DialogContent class='sm:max-w-[540px] max-h-[82vh] gap-0 px-6 pt-[18px] pb-[22px]' :show-close-button='false'>
        <DialogDescription class='sr-only'>Playback, keyboard, column and lookup preferences</DialogDescription>

        <div class='settings-head'>
            <DialogTitle>Settings</DialogTitle>
            <span class='head-actions'>
                <Button title='Copy log + settings for debugging' @click='copyDebugReport'>
                    <ClipboardListIcon class='size-3.5' />
                    Debug report
                </Button>
                <Button @click='$1t.send("openSettingsFolder")'>
                    <FolderOpenIcon class='size-3.5' />
                    Data folder
                </Button>
            </span>
        </div>

        <div class='settings-body'>
            <!-- Appearance -->
            <div class='ft-label section'>Appearance</div>
            <div class='hint'>Hover or focus to audition a theme live; click or press Enter to keep it.</div>
            <ThemePicker />

            <!-- Playback -->
            <div class='ft-label section'>Playback</div>
            <label class='row-item'>
                <span class='row-label'>Start playback when selecting a row <span class='sub'>off: Space, double-click and D play</span></span>
                <Switch v-model='$1t.settings.value.playOnSelect' @update:model-value='save' />
            </label>
            <label class='row-item'>
                <span class='row-label'>Keep playing when switching tracks</span>
                <Switch v-model='$1t.settings.value.continuePlayback' @update:model-value='save' />
            </label>
            <label class='row-item'>
                <span class='row-label'>Resume playback after seeking</span>
                <Switch v-model='$1t.settings.value.playOnSeek' @update:model-value='save' />
            </label>
            <label class='row-item'>
                <span class='row-label'>Advance to next track when playback ends</span>
                <Switch v-model='$1t.settings.value.autoPlayNext' @update:model-value='save' />
            </label>

            <!-- Keyboard -->
            <div class='ft-label section'>Keyboard</div>
            <div class='hint'>Click the keyboard icon to remap. Per-tag hotkeys are set in Manage tags.</div>
            <div v-for='action in FLOWTAG_ACTIONS' :key='action' class='row-item'>
                <span class='row-label'>{{ FLOWTAG_ACTION_LABELS[action] }}</span>
                <span class='bind'>
                    <span v-if='!$1t.settings.value.flowtag.keybinds[action]' class='ft-kbd'>{{ defaultKeyLabel(action) }}</span>
                    <Keybind
                        :key='action + "-" + nonce'
                        :model-value='$1t.settings.value.flowtag.keybinds[action]'
                        @update:model-value='(kb: any) => setBind(action, kb)'
                    />
                </span>
            </div>

            <!-- Columns -->
            <div class='ft-label section'>Track list columns</div>
            <div class='hint'>Title, artist and tags are always shown.</div>
            <label v-for='col in COLUMN_OPTIONS' :key='col.key' class='row-item'>
                <span class='row-label'>{{ col.label }}</span>
                <Switch v-model='$1t.settings.value.flowtag.columns[col.key]' @update:model-value='save' />
            </label>

            <!-- Lookup -->
            <div class='ft-label section'>Lookup (M)</div>
            <label class='row-item'>
                <span class='row-label'>Single-page platform settings</span>
                <Switch v-model='$1t.settings.value.autoTaggerSinglePage' @update:model-value='save' />
            </label>
            <label class='row-item'>
                <span class='row-label'>Show lookup profiles</span>
                <Switch v-model='$1t.settings.value.showAutoTaggerProfiles' @update:model-value='save' />
            </label>

            <!-- Experimental -->
            <div class='ft-label section'>Experimental</div>
            <label class='row-item'>
                <span class='row-label'>Audition analysis <span class='sub'>auto-gain, intro/drop markers, shift+space plays the drop</span></span>
                <Switch v-model='$1t.settings.value.flowtag.analysisEnabled' @update:model-value='save' />
            </label>
            <div v-if='$1t.settings.value.flowtag.analysisEnabled' class='row-item'>
                <span class='row-label'>
                    <template v-if='store.analysisRun.value.running'>
                        Analyzing… {{ store.analysisRun.value.done.toLocaleString() }} / {{ store.analysisRun.value.total.toLocaleString() }}
                    </template>
                    <template v-else>Analyze unprocessed tracks <span class='sub'>loudness needs one full decode per track</span></template>
                </span>
                <Button variant='default' :disabled='store.analysisRun.value.running' @click='runAnalysis'>
                    {{ store.analysisRun.value.running ? 'Running…' : 'Analyze library' }}
                </Button>
            </div>

            <!-- Tangle -->
            <div class='ft-label section'>Tangle</div>
            <TangleSettings />

            <!-- Advanced -->
            <div class='ft-label section'>Advanced</div>
            <label class='row-item'>
                <span class='row-label'>Webview devtools <span class='sub'>takes effect after restart</span></span>
                <Switch v-model='$1t.settings.value.devtools' @update:model-value='save' />
            </label>
            <label class='row-item'>
                <span class='row-label'>In-app folder browser</span>
                <Switch v-model='$1t.settings.value.nonNativeBrowser' @update:model-value='save' />
            </label>
            <label class='row-item'>
                <span class='row-label'>Open tracks in external audio player</span>
                <Switch v-model='$1t.settings.value.externalAudioPlayer' @update:model-value='save' />
            </label>
            <label class='row-item'>
                <span class='row-label'>Client-side player <span class='sub'>server mode</span></span>
                <Switch v-model='$1t.settings.value.clientSidePlayer' @update:model-value='save' />
            </label>
        </div>

    </DialogContent>
</Dialog>
</template>

<script lang='ts' setup>
import { ref, watch } from 'vue';
import { ClipboardListIcon, FolderOpenIcon } from '@lucide/vue';
import { notify } from '@/lib/notify';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog';
import { Switch } from '@/components/ui/switch';

import { get1t } from '../scripts/onetagger';
import { getTagStore } from '../scripts/tagstore';
import { FLOWTAG_ACTIONS, FlowTagAction } from '../scripts/settings';
import { FLOWTAG_ACTION_DEFAULTS, FLOWTAG_ACTION_LABELS, FIXED_FLOWTAG_CODES } from '../scripts/flowkeys';
import { Keybind as KeybindClass } from '../scripts/utils';
import { diag } from '../scripts/diag';
import Keybind from './Keybind.vue';
import TangleSettings from './flowtag/TangleSettings.vue';
import ThemePicker from './flowtag/ThemePicker.vue';

const props = defineProps({
    modelValue: { type: Boolean, required: true }
});
const $1t = get1t();
const store = getTagStore();
const open = ref(props.modelValue);
// Bumped to remount a recorder whose recording was rejected
const nonce = ref(0);
const emit = defineEmits(['close']);

/** Keybind.key form of a KeyboardEvent.code ('KeyJ' -> 'j', 'Digit1' -> '1') */
function normalizeCode(code: string): string {
    return code.toLowerCase().replace('key', '').replace('digit', '').replace('numpad', '');
}
const FIXED_KEYS = new Set(FIXED_FLOWTAG_CODES.map(normalizeCode));

const COLUMN_OPTIONS = [
    { key: 'year', label: 'Year' },
    { key: 'bpm', label: 'BPM' },
    { key: 'key', label: 'Key' },
    { key: 'path', label: 'Path' },
    { key: 'added', label: 'Date added' },
] as const;

function defaultKeyLabel(action: FlowTagAction): string {
    return FLOWTAG_ACTION_DEFAULTS[action].replace('Key', '');
}

/** The bind an action currently responds to (custom, or the modifier-less default) */
function effectiveBind(action: FlowTagAction): KeybindClass {
    const kb = $1t.settings.value.flowtag.keybinds[action];
    if (kb && kb.key) return KeybindClass.fromJson(kb);
    return KeybindClass.fromJson({ key: normalizeCode(FLOWTAG_ACTION_DEFAULTS[action]) });
}

function sameBind(a: { key?: string, ctrl?: boolean, alt?: boolean, shift?: boolean }, b: KeybindClass): boolean {
    return a.key === b.key && !!a.ctrl === !!b.ctrl && !!a.alt === !!b.alt && !!a.shift === !!b.shift;
}

function reject(message: string) {
    notify({ type: 'warning', message });
    nonce.value++;
}

function setBind(action: FlowTagAction, kb?: any) {
    const current = $1t.settings.value.flowtag.keybinds;
    // Reset to default
    if (!kb || !kb.key) {
        const next = { ...current };
        delete next[action];
        $1t.settings.value.flowtag.keybinds = next;
        save();
        return;
    }
    const hasModifier = !!(kb.ctrl || kb.alt || kb.shift);
    if (!hasModifier && FIXED_KEYS.has(String(kb.key))) {
        reject('That key is a fixed transport key (space, arrows, digits).');
        return;
    }
    for (const other of FLOWTAG_ACTIONS) {
        if (other !== action && sameBind(kb, effectiveBind(other))) {
            reject(`Already used by "${FLOWTAG_ACTION_LABELS[other]}".`);
            return;
        }
    }
    const clash = store.tags.value.find((t) => t.keybind && sameBind(kb, KeybindClass.fromJson(t.keybind)));
    if (clash) {
        reject(`Already used as the hotkey of tag "${clash.name}".`);
        return;
    }
    $1t.settings.value.flowtag.keybinds = { ...current, [action]: KeybindClass.fromJson(kb) };
    diag('keybindSet', { action, key: kb.key, ctrl: !!kb.ctrl, alt: !!kb.alt, shift: !!kb.shift });
    save();
}

/** One paste that lets an assistant debug a session: versions, redacted
 *  settings, and the tail of the interleaved app + client log. */
async function copyDebugReport() {
    $1t.send('getLog');
    const log: string = await new Promise((resolve) => {
        const stop = watch(() => $1t.info.value.log, (v) => { stop(); resolve(v ?? ''); });
        setTimeout(() => { stop(); resolve($1t.info.value.log ?? ''); }, 3000);
    });
    const tail = (log || '(log unavailable)').split('\n').slice(-400).join('\n');
    const settings = JSON.parse(JSON.stringify($1t.settings.value));
    settings.audioFeatures = '[redacted]';
    delete settings.autoTaggerConfig;
    delete settings.autoTaggerProfiles;
    const report = [
        '# FlowTag debug report',
        `generated: ${new Date().toISOString()}`,
        `version: ${$1t.info.value.version} (${$1t.info.value.commit})  os: ${$1t.info.value.os}`,
        '',
        '## settings',
        JSON.stringify(settings, null, 1),
        '',
        '## log (last 400 lines)',
        tail,
    ].join('\n');
    diag('debugReportCopied', { bytes: report.length });
    try {
        await navigator.clipboard.writeText(report);
        notify({ message: 'Debug report copied to clipboard', timeout: 1500 });
    } catch {
        // eslint-disable-next-line no-console
        console.log(report);
        notify({ type: 'warning', message: 'Clipboard unavailable — report printed to the devtools console' });
    }
}

function save() {
    diag('settingsChanged');
    $1t.saveSettings(false);
}

function runAnalysis() {
    diag('analysisRun');
    store.analysisRun.value = { running: true, done: 0, total: 0 };
    $1t.send('analysisRun');
}

function onOpenChange(value: boolean) {
    open.value = value;
    if (!value) close();
}

// Save on close
function close() {
    $1t.saveSettings(false);
    emit("close");
}

// Watch if opened or no
watch(props, () => {
    open.value = props.modelValue;
});
</script>

<style lang='scss' scoped>
.settings-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 4px;
    .head-actions { display: inline-flex; gap: 6px; }
}
.settings-body {
    overflow-y: auto;
    max-height: calc(82vh - 70px);
}
.section {
    margin: 20px 0 4px;
}
.hint {
    font-size: 11.5px;
    color: var(--ft-text-3);
    margin: 2px 0 4px;
}
.row-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 34px;
    border-bottom: 1px solid var(--ft-line);
    &:last-child { border-bottom: none; }
}
.row-label {
    font-size: 12.5px;
    color: var(--ft-text-2);
    .sub {
        font-size: 11px;
        color: var(--ft-text-3);
        margin-left: 6px;
    }
}
.bind {
    display: inline-flex;
    align-items: center;
    gap: 8px;
}
</style>

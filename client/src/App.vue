<template>
    <TooltipProvider :delay-duration='400'>
    <div>
        <Toaster position='bottom-right' :duration='2500' />
        <!-- FlowTag IS the app: no header, no tabs; the player docks at the bottom -->
        <div class='flex flex-col h-screen bg-ft-bg'>
            <main class='flex-1 min-h-0'>
                <router-view v-if='$1t.info.value.ready'></router-view>
                <div v-else class='flex h-full items-center justify-center'>
                    <LoaderCircleIcon class='size-10 animate-spin text-ft-text-3' :stroke-width='1.5' aria-label='Loading' />
                </div>
            </main>
            <footer v-if='!isMini' class='ft-footer flowtag-theme shrink-0'>
                <PlayerBar v-if='$1t.info.value.ready'></PlayerBar>
            </footer>
        </div>

        <!-- Settings (opened from the FlowTag header gear) -->
        <Settings v-model="settings" @close="settingsClosed"></Settings>

        <!-- Min size: blocks the app (and its shortcuts) until the window grows -->
        <AlertDialog :open='sizeDialog'>
            <AlertDialogContent @escape-key-down.prevent>
                <AlertDialogHeader>
                    <AlertDialogTitle>Window too small</AlertDialogTitle>
                    <AlertDialogDescription>FlowTag needs at least a 800×450 window. Resize to continue.</AlertDialogDescription>
                </AlertDialogHeader>
            </AlertDialogContent>
        </AlertDialog>

        <!-- confirm / prompt / alert requests from lib/dialogs -->
        <DialogHost />

        <!-- ⌘Enter: commit the staged tracks -->
        <CommitSheet />

        <!-- Done chip: today, chart and trail -->
        <JournalDialog />

        <!-- In-app folder browser (Settings → Advanced) -->
        <FolderBrowser />

        <!-- F3 -->
        <DevTools v-model='devToolsOpen' />

        <!-- Startup: the lookup needs the platform list -->
        <AlertDialog :open='$1t.platformsLoading.value && !isMini'>
            <AlertDialogContent class='sm:max-w-[280px]' @escape-key-down.prevent>
                <AlertDialogHeader class='flex-row items-center gap-3'>
                    <LoaderCircleIcon class='size-4 animate-spin text-ft-text-3' />
                    <AlertDialogTitle>Loading platforms…</AlertDialogTitle>
                </AlertDialogHeader>
                <AlertDialogDescription class='sr-only'>FlowTag is loading its metadata platforms</AlertDialogDescription>
            </AlertDialogContent>
        </AlertDialog>
    </div>
    </TooltipProvider>
</template>

<script lang='ts' setup>
import { computed, onMounted, ref, watch } from "vue";
import { useDebounceFn, useEventListener } from '@vueuse/core';
import { useRoute } from 'vue-router';
import { get1t } from "./scripts/onetagger.js";
import { initMediaSync } from './scripts/mediasync';
import { initDiag } from './scripts/diag';
import { applyTheme, DEFAULT_THEME } from './scripts/themes';
import { installKeycapEcho } from './scripts/keycapecho';
import { Toaster } from '@/components/ui/sonner';
import { TooltipProvider } from '@/components/ui/tooltip';
import { LoaderCircleIcon } from '@lucide/vue';
import { AlertDialog, AlertDialogContent, AlertDialogDescription, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog';
import DialogHost from '@/components/flowtag/DialogHost.vue';
import { MIN_WINDOW_H, MIN_WINDOW_W, waveformWaves } from '@/lib/layout';
import CommitSheet from '@/components/flowtag/CommitSheet.vue';
import JournalDialog from '@/components/flowtag/JournalDialog.vue';

import Settings from './components/Settings.vue';
import PlayerBar from './components/PlayerBar.vue';
import FolderBrowser from './components/FolderBrowser.vue';
import DevTools from './components/DevTools.vue';

const $1t = get1t();

const settings = ref(false);
const sizeDialog = ref(false);
const route = useRoute();
// The miniplayer window: no footer, no min-size demands, no media sync of its own
const isMini = computed(() => route.path === '/mini' || window.location.hash.startsWith('#/mini'));

// When settings closed
function settingsClosed() {
    settings.value = false;
    $1t.quickTagUnfocus();
}

// The FlowTag header's gear asks for the app-level settings dialog
function openSettings() {
    settings.value = true;
}

function setWaveformWidth() {
    document.documentElement.style.setProperty('--waveform-wave', waveformWaves(window.innerWidth).toString());
}

// Re-shaping the 180 variable-font waveform glyphs is the most expensive
// relayout on the page; WKWebView (macOS 26) defers reflow during a live
// resize and catches up when the drag pauses, so the glyphs re-shape only
// once the size settles, keeping that catch-up as short as possible.
const setWaveformWidthSettled = useDebounceFn(setWaveformWidth, 150);

function onResize() {
    setWaveformWidthSettled();
    sizeDialog.value = !isMini.value && (window.innerHeight < MIN_WINDOW_H || window.innerWidth < MIN_WINDOW_W);
}

// Setup
onMounted(() => {
    initDiag();
    setWaveformWidth();
    if (!isMini.value) initMediaSync();
    installKeycapEcho();
});

// The saved theme follows the settings (loaded over the websocket after
// boot); the first application is instant, later changes crossfade
let themeApplied = false;
watch(() => $1t.settings.value.flowtag.theme, (theme) => {
    applyTheme(theme ?? DEFAULT_THEME, themeApplied);
    themeApplied = true;
}, { immediate: true });

useEventListener(window, 'resize', onResize);
useEventListener(window, 'flowtag-open-settings', openSettings);

const devToolsOpen = ref(false);
useEventListener(window, 'flowtag-open-devtools', () => { devToolsOpen.value = true; });
</script>

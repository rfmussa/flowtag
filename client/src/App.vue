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
                    <AlertDialogDescription>FlowTag needs at least a 1200×550 window. Resize to continue.</AlertDialogDescription>
                </AlertDialogHeader>
            </AlertDialogContent>
        </AlertDialog>

        <!-- confirm / prompt / alert requests from lib/dialogs -->
        <DialogHost />

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
import { computed, onMounted, ref } from "vue";
import { useEventListener } from '@vueuse/core';
import { useRoute } from 'vue-router';
import { get1t } from "./scripts/onetagger.js";
import { initMediaSync } from './scripts/mediasync';
import { initDiag } from './scripts/diag';
import { Toaster } from '@/components/ui/sonner';
import { TooltipProvider } from '@/components/ui/tooltip';
import { LoaderCircleIcon } from '@lucide/vue';
import { AlertDialog, AlertDialogContent, AlertDialogDescription, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog';
import DialogHost from '@/components/flowtag/DialogHost.vue';

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
    document.documentElement.style.setProperty('--waveform-wave', Math.min(Math.round(20 + ((window.innerWidth - 1200) / 10)), 70).toString());
}

function onResize() {
    setWaveformWidth();
    sizeDialog.value = !isMini.value && (window.innerHeight < 550 || window.innerWidth < 1200);
}

// Setup
onMounted(() => {
    initDiag();
    setWaveformWidth();
    if (!isMini.value) initMediaSync();
});

useEventListener(window, 'resize', onResize);
useEventListener(window, 'flowtag-open-settings', openSettings);

const devToolsOpen = ref(false);
useEventListener(window, 'flowtag-open-devtools', () => { devToolsOpen.value = true; });
</script>

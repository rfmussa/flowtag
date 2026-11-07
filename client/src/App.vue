<template>
    <div>
        <q-layout view="hHh lpR fFf" class="bg-background">
            <!-- FlowTag IS the app: no header, no tabs -->
            <q-page-container class="content">
                <router-view v-if='$1t.info.value.ready'></router-view>
                <!-- Loading -->
                <div v-if='!$1t.info.value.ready' class='row justify-center items-center' style='height: 100vh'>
                    <q-circular-progress indeterminate color='primary' size='64px'></q-circular-progress>
                </div>
            </q-page-container>

            <!-- Player -->
            <q-footer class="flowtag-theme">
                <PlayerBar v-if='$1t.info.value.ready'></PlayerBar>
            </q-footer>
        </q-layout>

        <!-- Settings (opened from the FlowTag header gear) -->
        <Settings v-model="settings" @close="settingsClosed"></Settings>

        <!-- Min size dialog -->
        <q-dialog v-model="sizeDialog" persistent>
            <q-card class='flowtag-theme'>
                <q-card-section>
                    <div class="text-h6">Warning</div>
                </q-card-section>
                <q-card-section>
                    FlowTag requires at least a 1200x550 window. Please
                    resize to continue.
                </q-card-section>
            </q-card>
        </q-dialog>

        <!-- Folder browser dialog -->
        <q-dialog v-model='$1t.folderBrowser.value.open'>
            <FolderBrowser v-if='$1t.folderBrowser.value.open' :base='$1t.folderBrowser.value.basePath'></FolderBrowser>
        </q-dialog>
    </div>
</template>

<script lang='ts' setup>
import { useQuasar } from 'quasar';
import { onMounted, onUnmounted, ref } from "vue";
import { get1t } from "./scripts/onetagger.js";
import { initMediaSync } from './scripts/mediasync';

import Settings from './components/Settings.vue';
import PlayerBar from './components/PlayerBar.vue';
import FolderBrowser from './components/FolderBrowser.vue';

const $1t = get1t();
const $q = useQuasar();

const settings = ref(false);
const sizeDialog = ref(false);

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
    sizeDialog.value = window.innerHeight < 550 || window.innerWidth < 1200;
}

// Setup
onMounted(() => {
    $q.dark.set(true);
    setWaveformWidth();
    initMediaSync();
    window.addEventListener("resize", onResize);
    window.addEventListener('flowtag-open-settings', openSettings);
});

onUnmounted(() => {
    window.removeEventListener("resize", onResize);
    window.removeEventListener('flowtag-open-settings', openSettings);
});
</script>

<style lang='scss'>
.content {
    overflow-y: auto !important;
    height: calc(100vh);
    min-height: 100vh;
}
</style>

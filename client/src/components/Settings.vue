<template>
<q-dialog v-model='open' @hide='close()'>

    <q-card flat class='parent-card bg-dark q-pa-lg'>
        <!-- Header -->
        <q-card-section>            
            <div class='text-subtitle2 text-bold q-mb-sm text-center text-uppercase text-primary'>Settings</div>
            <q-tabs v-model='tab'>
                <q-tab label='Preferences' class='q-pa-lg' name='advanced'></q-tab>
            </q-tabs>
        </q-card-section>

        <!-- Content -->
        <q-card-section>
            <!-- Advanced -->
            <div v-if='tab == "advanced"'>
                
                <!-- Open settings folder -->
                <div class='text-center q-mb-xl'>
                <q-btn dense push
                    color='primary'
                    class='rounded-borders q-px-md q-mt-xs text-weight-medium text-black'
                    @click='$1t.send("openSettingsFolder")'
                >Open data folder</q-btn>
                </div>
                
                <q-separator class='q-mx-auto' :style='"max-width: 513px; margin-top: -8px; margin-bottom: 35px"' inset color="darker"/>
                
                <div class='text-uppercase text-primary text-subtitle2 text-bold q-mt-lg q-mb-xs text-left'>Auto Tag</div>
                <q-checkbox
                    v-model='$1t.settings.value.autoTaggerSinglePage'
                    label="Show as single page"
                    class='checkbox'
                ></q-checkbox><br>
                <q-checkbox
                    v-model='$1t.settings.value.showAutoTaggerProfiles'
                    label="Show profiles"
                    class='checkbox'
                ></q-checkbox><br>
                <div class='text-uppercase text-primary text-subtitle2 text-bold q-mt-lg q-mb-xs text-left'>Quick Tag</div>
                <q-checkbox
                    v-model='$1t.settings.value.continuePlayback'
                    label='Continue playback when switching to a different track'
                    class='checkbox'
                ></q-checkbox><br>
                <q-checkbox
                    v-model='$1t.settings.value.playOnSeek'
                    label='Start/continue playback after seeking'
                    class='checkbox'
                ></q-checkbox><br>
                <q-checkbox
                    v-model='$1t.settings.value.autoPlayNext'
                    label='Go to next track when playback ends'
                    class='checkbox'
                ></q-checkbox><br>
                <q-checkbox
                    v-model='$1t.settings.value.externalAudioPlayer'
                    label='Use external audio player'
                    class='checkbox'
                ></q-checkbox><br>

                <div class='text-uppercase text-primary text-subtitle2 text-bold q-mt-lg q-mb-xs text-left'>Edit Tags</div>
                <q-checkbox
                    v-model='$1t.settings.value.tagEditorDouble'
                    label="Show 'Your list'"
                    class='checkbox'
                ></q-checkbox><br>
                <q-checkbox
                    v-model='$1t.settings.value.tagEditorAutosave'
                    label='Autosave changes when switching to a different track'
                    class='checkbox'
                ></q-checkbox><br>
                <q-checkbox
                    v-model='$1t.settings.value.tagEditorPlayer'
                    label='Show player in tag editor'
                    class='checkbox'
                ></q-checkbox><br>

                <div class='text-uppercase text-primary text-subtitle2 text-bold q-mt-lg q-mb-xs text-left'>Display</div>
                <q-checkbox
                    v-model='$1t.settings.value.helpButton'
                    label='Show help button'
                    class='checkbox'
                ></q-checkbox>     
                
                <div class='text-uppercase text-primary text-subtitle2 text-bold q-mt-lg q-mb-xs text-left'>Advanced</div>
                <q-checkbox
                    v-model='$1t.settings.value.clientSidePlayer'
                    label='Client side player (for server mode)'
                    class='checkbox'
                ></q-checkbox><br>
                <q-checkbox
                    v-model='$1t.settings.value.nonNativeBrowser'
                    label='Client side folder browser'
                    class='checkbox'
                ></q-checkbox>    
                        
                <!-- Color picker -->
                <div class='text-uppercase text-primary text-subtitle2 text-bold q-mt-lg q-mb-md text-left'>Primary color</div>
                <q-color 
                    v-model='$1t.settings.value.primaryColor'
                    @change='colorChange'
                    flat
                ></q-color>
                <q-btn
                    @click='$1t.settings.value.primaryColor = "#00d2bf"; colorChange()'
                    color='primary'
                    flat
                    class='q-mt-sm'
                >
                    Reset color to default
                </q-btn>
            </div>

        </q-card-section>
    </q-card>

</q-dialog>
</template>

<script lang='ts' setup>
import { ref, watch } from 'vue';
import { get1t } from '../scripts/onetagger';
import { setCssVar } from 'quasar';

const props = defineProps({
    modelValue: { type: Boolean, required: true }
});
const $1t = get1t();
const open = ref(props.modelValue);
const tab = ref('advanced');
const emit = defineEmits(['close']);

// Primary color change
function colorChange() {
    setCssVar('primary', $1t.settings.value.primaryColor);
}

// Save on close
function close() {
    $1t.saveSettings();
    emit("close");
}

// Watch if opened or no
watch(props, () => {
    open.value = props.modelValue;
});


</script>

<style>
.parent-card {
    max-height: 80vh;
    height: 78vh;
    width: 70vw;
    min-width: 600px;
}
</style>
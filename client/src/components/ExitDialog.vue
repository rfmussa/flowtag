<template>
<q-dialog ref='dialogRef'>
    <q-card class='q-dialog-plugin'>

        <q-card-section>
            <div class='text-h6'>Tagging in progress</div>
        </q-card-section>

        <q-card-section>
            <div>Tagging is in progress, do you really want to exit?</div>
        </q-card-section>

        <q-card-section class='row'>
            <q-space></q-space>
            <q-btn color='white' flat @click='onDialogCancel()'>Cancel</q-btn>
            <q-btn color='red' flat @click='exit()'>Exit anyway</q-btn>
        </q-card-section>

    </q-card>
</q-dialog>
</template>

<script lang='ts' setup>
import { useDialogPluginComponent } from 'quasar';
import { wsUrl } from '../scripts/utils';

const { dialogRef, onDialogCancel } = useDialogPluginComponent();

defineEmits([...useDialogPluginComponent.emits]);

// Exit
function exit() {
    // Use another connection because main thread is locked in AT
    let ws = new WebSocket(wsUrl());
    ws.onopen = () => {
        ws.send(JSON.stringify({action: 'exit'}));
    }
}

</script>

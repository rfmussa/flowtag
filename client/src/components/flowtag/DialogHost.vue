<template>
<AlertDialog :open='!!request'>
    <AlertDialogContent v-if='request' :key='request.id' @open-auto-focus='focusFirst' @escape-key-down='dismiss'>
        <AlertDialogHeader>
            <AlertDialogTitle>{{ request.title }}</AlertDialogTitle>
            <AlertDialogDescription v-if='request.message' class='break-words'>{{ request.message }}</AlertDialogDescription>
        </AlertDialogHeader>
        <form v-if='request.kind === "prompt"' @submit.prevent='ok'>
            <Input ref='inputEl' v-model='text' :placeholder='request.placeholder' spellcheck='false' />
        </form>
        <AlertDialogFooter>
            <AlertDialogCancel v-if='request.cancelLabel' @click='cancel'>{{ request.cancelLabel }}</AlertDialogCancel>
            <AlertDialogAction :class='request.destructive ? "text-ft-danger" : ""' @click='ok'>{{ request.okLabel }}</AlertDialogAction>
        </AlertDialogFooter>
    </AlertDialogContent>
</AlertDialog>
</template>

<script lang='ts' setup>
import { nextTick, ref, watch } from 'vue';
import {
    AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
    AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Input } from '@/components/ui/input';
import { currentDialog as request, settleDialog } from '@/lib/dialogs';

const text = ref('');
const inputEl = ref<InstanceType<typeof Input>>();

watch(request, (r) => { text.value = r?.value ?? ''; }, { immediate: true });

/** Prompts focus (and select) their field; confirms keep Reka's default (Cancel first) */
function focusFirst(e: Event) {
    if (request.value?.kind !== 'prompt') return;
    e.preventDefault();
    nextTick(() => {
        const el = (inputEl.value?.$el ?? null) as HTMLInputElement | null;
        el?.focus();
        el?.select();
    });
}

function ok() {
    const r = request.value;
    if (r) settleDialog(r.id, r.kind === 'prompt' ? text.value : true);
}
function cancel() {
    const r = request.value;
    if (r) settleDialog(r.id, r.kind === 'alert');
}
/** Esc: alerts just close; confirms and prompts settle as dismissed */
function dismiss() {
    const r = request.value;
    if (r) settleDialog(r.id, r.kind === 'alert' ? true : null);
}
</script>

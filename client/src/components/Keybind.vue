<template>
<div>
    <!-- Current bind (click to change) -->
    <button
        v-if='isSet'
        type='button'
        class='keybind-caps inline-flex items-center gap-1 h-7 px-1.5 rounded-md cursor-pointer hover:bg-ft-hover'
        :aria-label='`Hotkey ${describe(value)}, click to change`'
        @click='overlay = true'
    >
        <kbd v-for='cap in caps(value)' :key='cap' class='ft-kbd'>{{ cap }}</kbd>
    </button>
    <Hint v-else text='Set hotkey'>
        <Button size='icon-sm' aria-label='Set hotkey' @click='overlay = true'><KeyboardIcon class='size-4' /></Button>
    </Hint>

    <!-- Recorder -->
    <Dialog v-model:open='overlay' @update:open='(o: boolean) => { if (!o) key.clear(); }'>
        <DialogContent class='sm:max-w-[300px]' @keydown='keydown'>
            <DialogHeader>
                <DialogTitle>Set keybind</DialogTitle>
                <DialogDescription>Press a letter, digit or F-key, with modifiers if you like.</DialogDescription>
            </DialogHeader>
            <div class='flex items-center justify-center gap-1.5 min-h-[52px] rounded-md border border-ft-line bg-ft-panel' aria-live='polite'>
                <kbd v-for='cap in caps(key)' :key='cap' class='ft-kbd keybind-preview'>{{ cap }}</kbd>
                <span v-if='!key.key' class='text-xs text-ft-text-3'>Waiting for a key…</span>
            </div>
            <DialogFooter>
                <Button v-if='isSet' variant='destructive' class='mr-auto' @click='reset'>Reset</Button>
                <Button @click='close'>Cancel</Button>
                <Button variant='default' :disabled='!key.key' @click='set'>Save</Button>
            </DialogFooter>
        </DialogContent>
    </Dialog>
</div>
</template>

<script lang='ts' setup>
import { computed, ref, watch } from 'vue';
import { KeyboardIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import Hint from './flowtag/Hint.vue';
import { Keybind } from '../scripts/utils.js';

const props = defineProps({
    modelValue: Object
});
const overlay = ref(false);
const key = ref(new Keybind());
const value = ref<Keybind | undefined>();
const emit = defineEmits(['update:modelValue']);

function keydown(e: KeyboardEvent) {
    // Record letters, digits and F-keys; everything else (Esc, Tab...) keeps its meaning
    if (e.code.match(/F\d{1,2}/) || e.code.startsWith('Key') || e.code.startsWith('Digit') || e.code.startsWith('Numpad')) {
        key.value.key = e.code.toLowerCase().replace('key', '').replace('digit', '').replace('numpad', '');
        key.value.ctrl = (e.ctrlKey || e.metaKey);
        key.value.alt = e.altKey;
        key.value.shift = e.shiftKey;
        e.preventDefault();
    }
}

// Close popup and reset
function close() {
    overlay.value = false;
    key.value.clear();
}

// Save
function set() {
    emit('update:modelValue', Object.assign(new Keybind(), key.value));
    value.value = Object.assign(new Keybind(), key.value);
    close();
}

// Callback with null
function reset() {
    emit('update:modelValue', undefined);
    value.value = undefined;
    close();
}

/** Keycap labels, modifiers first in macOS order (⌘ stands for ctrl or cmd: Keybind.ctrl stores either) */
function caps(k?: Keybind): string[] {
    if (!k || !k.key) return [];
    return [
        ...(k.ctrl ? ['⌘'] : []),
        ...(k.alt ? ['⌥'] : []),
        ...(k.shift ? ['⇧'] : []),
        String(k.key).toUpperCase(),
    ];
}

function describe(k?: Keybind): string {
    return caps(k).join(' ');
}

const isSet = computed(() => !!value.value?.key);

watch(() => props.modelValue, (v) => {
    value.value = v ? Object.assign(new Keybind(), v) : undefined;
}, { immediate: true, deep: true });
</script>

<style>
.flowtag-theme .ft-kbd.keybind-preview {
    font-size: 13px;
    padding: 5px 8px;
    color: var(--ft-text);
}
</style>

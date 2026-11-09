<template>
<Dialog v-model:open='$1t.folderBrowser.value.open'>
    <DialogContent class='flex flex-col gap-3 sm:max-w-[520px] h-[min(80vh,600px)]'>
        <DialogHeader>
            <DialogTitle>Choose a folder</DialogTitle>
            <DialogDescription>Open folders to move in; pick the one to add.</DialogDescription>
        </DialogHeader>

        <div class='flex-1 min-h-0 overflow-y-auto rounded-md border border-ft-line bg-ft-bg p-1' role='listbox' aria-label='Folders'>
            <button v-if='hasParent' type='button' class='folder-row' @click='go("..")'>
                <CornerLeftUpIcon class='size-3.5 text-ft-text-3' /><span class='text-ft-text-3'>Parent folder</span>
            </button>
            <div v-if='loading' class='flex items-center gap-2 px-2 py-2 text-xs text-ft-text-3'>
                <LoaderCircleIcon class='size-3.5 animate-spin' />Loading…
            </div>
            <div v-else-if='children.length === 0' class='px-2 py-2 text-xs text-ft-text-3'>No subfolders</div>
            <button
                v-for='child in children'
                :key='child.path'
                type='button'
                role='option'
                class='folder-row'
                :aria-label='`Open ${child.name}`'
                @click='go(child.name)'
            >
                <FolderIcon class='size-3.5 text-ft-text-3' /><span class='truncate'>{{ child.name }}</span>
            </button>
        </div>

        <form class='flex items-center gap-2' @submit.prevent='save'>
            <Input v-model='path' aria-label='Folder path' spellcheck='false' class='flex-1 font-mono text-xs' @keydown.enter.prevent='go("", path)' />
            <Button @click='$1t.folderBrowser.value.open = false'>Cancel</Button>
            <Button type='submit' variant='default' :disabled='!path'>Choose</Button>
        </form>
    </DialogContent>
</Dialog>
</template>

<script lang='ts' setup>
import { computed, ref, watch } from 'vue';
import { CornerLeftUpIcon, FolderIcon, LoaderCircleIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { get1t } from '../scripts/onetagger.js';

/**
 * In-app folder picker (Settings → Advanced → In-app folder browser).
 * The backend lists one folder per request; '..' resolves to the parent.
 */
const $1t = get1t();
const path = ref('/');
const entries = ref<{ path: string }[]>([]);
const loading = ref(false);

const children = computed(() => entries.value.map((e) => ({
    path: e.path,
    name: e.path.replace(/\\/g, '/').split('/').filter(Boolean).pop() ?? e.path,
})));
const hasParent = computed(() => path.value.replace(/\\/g, '/') !== '/');

/** List `child` inside `from` (default: the current folder) */
function go(child: string, from = path.value) {
    loading.value = true;
    $1t.send('folderBrowser', { path: from, child, base: false });
}

function save() {
    $1t.onBrowse({ context: $1t.folderBrowser.value.context, path: path.value, action: 'browse' });
    $1t.folderBrowser.value.open = false;
}

// Replies arrive through the app's socket handler
$1t.onFolderBrowserEvent = (json: any) => {
    if (json.action !== 'folderBrowser') return;
    loading.value = false;
    path.value = json.path;
    entries.value = json.entry?.children ?? [];
};

watch(() => $1t.folderBrowser.value.open, (open) => {
    if (!open) return;
    entries.value = [];
    go('', $1t.folderBrowser.value.basePath || '/');
}, { immediate: true });
</script>

<style lang='scss' scoped>
.folder-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 28px;
    padding: 0 8px;
    border-radius: 6px;
    font-size: 12.5px;
    color: var(--ft-text-2);
    text-align: left;
    cursor: pointer;
    &:hover { background: var(--ft-hover); color: var(--ft-text); }
}
</style>

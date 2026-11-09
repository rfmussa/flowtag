<template>
<div class='flex items-center'>
    <Popover>
        <PopoverTrigger as-child>
            <Button class='px-2'><FolderIcon />Folders</Button>
        </PopoverTrigger>
        <!-- No auto-focus: the first focusable is a delete button -->
        <PopoverContent align='start' class='w-[360px] p-1' @open-auto-focus.prevent>
            <div v-if='store.folders.value.length === 0' class='px-2 py-2 text-xs text-ft-text-3'>No folders added</div>
            <ul v-else class='divide-y divide-ft-line-faint'>
                <li v-for='folder in store.folders.value' :key='folder.id' class='flex items-center gap-2 h-8 pl-2'>
                    <span class='flex-1 min-w-0 truncate text-xs text-ft-text-2' :title='folder.path'>{{ folder.path }}</span>
                    <Hint text='Remove folder'>
                        <Button size='icon-sm' variant='destructive' :aria-label='`Remove ${folder.path}`' @click='remove(folder.id)'>
                            <TrashIcon class='size-3.5' />
                        </Button>
                    </Hint>
                </li>
            </ul>
            <div class='border-t border-ft-line mt-1 pt-1'>
                <Button size='sm' class='text-ft-text' @click='add'><FolderPlusIcon />Add folder</Button>
            </div>
        </PopoverContent>
    </Popover>

    <Button class='px-2 ml-1' :disabled='store.scan.value.running' @click='rescan'>
        <RefreshCwIcon />Rescan
    </Button>
</div>
</template>

<script lang='ts' setup>
import { FolderIcon, FolderPlusIcon, RefreshCwIcon, TrashIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { confirmDialog } from '@/lib/dialogs';
import Hint from './Hint.vue';
import { get1t } from '../../scripts/onetagger';
import { getTagStore } from '../../scripts/tagstore';

const store = getTagStore();
const $1t = get1t();

async function remove(id: number) {
    const folder = store.folders.value.find((f) => f.id === id);
    const count = store.tracks.value.filter((t) => t.path.startsWith((folder?.path ?? '') + '/')).length;
    const ok = await confirmDialog({
        title: 'Remove folder?',
        message: `${folder?.path ?? ''} — removes ${count} track(s) from the library, `
            + 'including tags not yet written to their files. Audio files are not touched.',
        okLabel: 'Remove',
        destructive: true,
    });
    if (ok) $1t.send('tagFolderRemove', { id });
}

// Native picker, or the in-app browser (Settings → Advanced); the reply lands in onBrowse
function add() {
    $1t.browse('tagFolderAdd');
}

function rescan() {
    $1t.send('tagLibraryScan', { full: false });
}
</script>

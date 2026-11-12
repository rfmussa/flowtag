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
                    <span v-if='counts(folder.id)' class='text-[10.5px] text-ft-text-3 ft-num shrink-0' data-test='folder-counts'>
                        {{ counts(folder.id)![0].toLocaleString() }}/{{ counts(folder.id)![1].toLocaleString() }}
                    </span>
                    <DropdownMenu>
                        <DropdownMenuTrigger as-child>
                            <Button size='icon-sm' :aria-label='`Folder menu for ${folder.path}`'>
                                <EllipsisIcon class='size-3.5' />
                            </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align='end' class='min-w-[140px]'>
                            <DropdownMenuItem :disabled='stageableIds(folder.id).length === 0' @select='stageFolder(folder.id)'>
                                Stage its not-done tracks ({{ stageableIds(folder.id).length }})
                            </DropdownMenuItem>
                        </DropdownMenuContent>
                    </DropdownMenu>
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
import { EllipsisIcon, FolderIcon, FolderPlusIcon, RefreshCwIcon, TrashIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { confirmDialog } from '@/lib/dialogs';
import Hint from './Hint.vue';
import { get1t } from '../../scripts/onetagger';
import { getTagStore } from '../../scripts/tagstore';

/** Tracks staged per "Stage its not-done tracks" action */
const STAGE_BATCH = 200;
const store = getTagStore();
const $1t = get1t();

/** [done, total] for a folder; absent when the server reported none */
function counts(folderId: number): [number, number] | undefined {
    return store.folderCounts.value.get(folderId);
}

function stageableIds(folderId: number): number[] {
    return store.folderNotDoneUnstagedIds(folderId, STAGE_BATCH);
}

function stageFolder(folderId: number) {
    store.stageTrackIds(stageableIds(folderId));
}

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

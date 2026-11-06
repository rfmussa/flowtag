<template>
<div class='row items-center no-wrap'>
    <q-btn dense flat no-caps icon='mdi-folder' label='Folders' class='q-px-sm'>
        <q-menu class='bg-darker' style='min-width: 360px'>
            <q-list dense separator>
                <q-item v-if='store.folders.value.length === 0'>
                    <q-item-section class='text-grey-6 text-caption'>No folders added</q-item-section>
                </q-item>
                <q-item v-for='folder in store.folders.value' :key='folder.id'>
                    <q-item-section class='ellipsis text-caption' :title='folder.path'>{{ folder.path }}</q-item-section>
                    <q-item-section side>
                        <q-btn flat round dense size='sm' icon='mdi-delete' color='red' @click='remove(folder.id)'>
                            <q-tooltip>Remove folder</q-tooltip>
                        </q-btn>
                    </q-item-section>
                </q-item>
            </q-list>
            <div class='q-pa-sm'>
                <q-btn dense flat no-caps icon='mdi-folder-plus' label='Add folder' color='primary' @click='add'></q-btn>
            </div>
        </q-menu>
    </q-btn>

    <q-btn
        dense
        flat
        no-caps
        icon='mdi-refresh'
        label='Rescan'
        class='q-px-sm q-ml-xs'
        :disable='store.scan.value.running'
        @click='rescan'
    ></q-btn>
</div>
</template>

<script lang='ts' setup>
import { get1t } from '../../scripts/onetagger';
import { getTagStore } from '../../scripts/tagstore';

const store = getTagStore();
const $1t = get1t();

function remove(id: number) {
    $1t.send('tagFolderRemove', { id });
}

// Async: the main app wires the 'browse' reply (context 'tagFolderAdd') to tagFolderAdd
function add() {
    $1t.send('browse', { context: 'tagFolderAdd' });
}

function rescan() {
    $1t.send('tagLibraryScan', { full: false });
}
</script>

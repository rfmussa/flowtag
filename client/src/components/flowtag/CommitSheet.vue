<template>
<Dialog :open='commitSheetOpen' @update:open='onOpenChange'>
    <DialogContent class='sm:max-w-[480px] gap-0 px-[22px] pt-[18px] pb-4' @keydown.enter='onEnter'>
        <DialogHeader>
            <DialogTitle>Commit staged tracks</DialogTitle>
            <DialogDescription class='mt-1.5'>
                Marks the ticked tracks done and records them as one commit.
            </DialogDescription>
        </DialogHeader>

        <div class='list'>
            <label v-for='row in rows' :key='row.trackId' class='row' :class='{ unfinished: !row.ready }'>
                <Checkbox :model-value='!!ticks[row.trackId]' @update:model-value='ticks = toggleTick(ticks, row.trackId)' />
                <span class='title'>{{ label(row.trackId).title }}</span>
                <span class='artist'>{{ label(row.trackId).artist }}</span>
                <span class='marker' :data-kind='markerOf(row)'>{{ markerOf(row) === 'skip' ? 'skip' : markerOf(row) === 'ready' ? 'ready' : 'not finished' }}</span>
            </label>
        </div>

        <Input v-model='message' class='mt-3' placeholder='What were you doing?' spellcheck='false' />

        <DialogFooter class='mt-4'>
            <Button @click='close'>Cancel</Button>
            <Button variant='default' :disabled='payload.trackIds.length === 0' @click='confirm'>
                Commit {{ payload.trackIds.length }} of {{ rows.length }}
            </Button>
        </DialogFooter>
    </DialogContent>
</Dialog>
</template>

<script lang='ts' setup>
import { computed, ref, watch } from 'vue';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { commitPayload, initialTicks, toggleTick, Ticks } from '@/lib/commit';
import { notify } from '@/lib/notify';
import { commitSheetOpen } from '@/scripts/commitsheet';
import { getTagStore, StageRow } from '@/scripts/tagstore';

const store = getTagStore();
const rows = computed(() => store.stageRows.value);
const ticks = ref<Ticks>({});
const message = ref('');
/** Set when this sheet sent a commit, so only its confirmation toasts */
let awaitingCommit = false;

// Fresh ticks and an empty message every time the sheet opens
watch(commitSheetOpen, (open) => {
    if (!open) return;
    ticks.value = initialTicks(rows.value);
    message.value = '';
});

const payload = computed(() => commitPayload(rows.value, ticks.value, message.value));

function label(trackId: number) {
    const t = store.tracks.value.find((x) => x.id === trackId);
    return { title: t?.title || t?.path.split(/[\\/]/).pop() || `Track ${trackId}`, artist: t?.artist ?? '' };
}

function markerOf(row: StageRow): 'skip' | 'ready' | 'unfinished' {
    if (row.skip) return 'skip';
    return row.ready ? 'ready' : 'unfinished';
}

function close() { commitSheetOpen.value = false; }
function onOpenChange(open: boolean) { commitSheetOpen.value = open; }

function confirm() {
    if (payload.value.trackIds.length === 0) return;
    awaitingCommit = true;
    store.commitStage(payload.value.trackIds, payload.value.message);
    close();
}

/** Enter confirms from anywhere but a focused button (which would click itself) */
function onEnter(e: KeyboardEvent) {
    if ((e.target as HTMLElement).tagName === 'BUTTON') return;
    e.preventDefault();
    confirm();
}

watch(() => store.lastCommit.value, (commit) => {
    if (!commit || !awaitingCommit) return;
    awaitingCommit = false;
    notify({ message: `Committed ${commit.trackCount} track${commit.trackCount === 1 ? '' : 's'}`, type: 'positive', timeout: 2000 });
});
</script>

<style lang='scss' scoped>
.list { margin-top: 14px; max-height: 320px; overflow-y: auto; }
.row {
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 32px;
    padding: 0 8px;
    border-radius: 6px;
    font-size: 12.5px;
    color: var(--ft-text);
    cursor: default;
    &:hover { background: var(--ft-hover); }
    &.unfinished { color: var(--ft-text-2); }
    .title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .artist { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ft-text-3); }
    .marker { flex-shrink: 0; font-size: 10.5px; color: var(--ft-text-3); }
    .marker[data-kind='unfinished'] { color: var(--ft-danger); }
}
</style>

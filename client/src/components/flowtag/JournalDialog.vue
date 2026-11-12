<template>
<Dialog :open='journalOpen' @update:open='journalOpen = $event'>
    <DialogContent class='sm:max-w-[560px] gap-0 px-[22px] pt-[18px] pb-4' data-test='journal'>
        <DialogHeader>
            <DialogTitle>Journal</DialogTitle>
            <DialogDescription class='mt-1.5'>
                {{ store.libraryDone.value.toLocaleString() }} done in library
            </DialogDescription>
        </DialogHeader>

        <div v-if='!journal' class='empty'>Loading…</div>
        <div v-else class='body'>
            <div class='today' data-test='journal-today'>
                <div class='stat'><span class='num ft-num'>{{ journal.today.committed }}</span><span class='cap'>committed today</span></div>
                <div class='stat'><span class='num ft-num'>{{ journal.today.tagsCreated }}</span><span class='cap'>tags created</span></div>
                <div class='stat'>
                    <span class='num ft-num'>{{ formatActive(journal.today.activeSecs) }}</span>
                    <span class='cap'>active · idle time not counted</span>
                </div>
            </div>

            <div class='chart' data-test='journal-chart'>
                <div v-for='(bar, i) in bars' :key='journal.days[i][0]' class='bar'
                    :title='`${journal.days[i][0]} · ${journal.days[i][1]}`'>
                    <div class='fill' :style='{ height: `${Math.max(bar * 100, bar > 0 ? 4 : 0)}%` }' />
                </div>
            </div>

            <div class='section-label'>Commits</div>
            <div v-if='journal.commits.length === 0' class='empty'>No commits yet</div>
            <div v-for='c in journal.commits' :key='c.id' class='line commit' data-test='journal-commit'>
                <span class='text'>{{ c.message || '(no message)' }}</span>
                <span class='meta ft-num'>{{ c.trackCount }} track{{ c.trackCount === 1 ? '' : 's' }} · {{ timeOf(c.createdAt) }}</span>
                <Button size='sm' @click='revertCommit(c)'>Revert</Button>
            </div>

            <div class='section-label'>Trail</div>
            <div v-if='groups.length === 0' class='empty'>Nothing yet</div>
            <div v-for='g in groups' :key='g.day'>
                <div class='day'>{{ g.day }}</div>
                <div v-for='l in g.lines' :key='l.id' class='line' data-test='journal-line'>
                    <span class='meta ft-num'>{{ timeOf(l.at) }}</span>
                    <span class='text'>{{ l.line.text }}</span>
                    <Button v-if='l.line.undo' size='sm' @click='undo(l.line.undo)'>Undo</Button>
                </div>
            </div>
            <Button v-if='canLoadMore' class='mt-2' data-test='journal-more' @click='loadMore'>Load more</Button>
        </div>
    </DialogContent>
</Dialog>
</template>

<script lang='ts' setup>
import { computed } from 'vue';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { confirmDialog } from '@/lib/dialogs';
import { chartBars, CommitRow, DayGroup, formatActive, groupTrail } from '@/lib/journal';
import { get1t } from '@/scripts/onetagger';
import { journalOpen } from '@/scripts/journalsheet';
import { getTagStore } from '@/scripts/tagstore';

const PAGE_SIZE = 100;
const store = getTagStore();
const journal = computed(() => store.journal.value);
const bars = computed(() => chartBars(journal.value?.days ?? []));
const canLoadMore = computed(() => {
    const n = journal.value?.trail.length ?? 0;
    return n > 0 && n % PAGE_SIZE === 0;
});

const dayOf = (secs: number) => new Date(secs * 1000).toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' });
const timeOf = (secs: number) => new Date(secs * 1000).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });

const groups = computed<DayGroup[]>(() => groupTrail(journal.value?.trail ?? [], journal.value?.commits ?? [], dayOf));

/** The server's broadcasts (stageState, trackTagsChanged) patch the rows; only the trail refreshes */
function undo(op: { action: string; data: any }) {
    get1t().send(op.action, op.data);
    store.requestJournal();
}

async function revertCommit(c: CommitRow) {
    const ok = await confirmDialog({
        title: 'Revert this commit?',
        message: `${c.trackCount} track${c.trackCount === 1 ? '' : 's'} go back to the stage.`,
        okLabel: 'Revert',
        destructive: true,
    });
    if (!ok) return;
    get1t().send('stageRevert', { commitId: c.id });
    get1t().send('tagTracksLoad');
    store.requestJournal();
}

function loadMore() {
    const trail = journal.value?.trail ?? [];
    if (trail.length === 0) return;
    store.requestJournal(trail[trail.length - 1].id);
}
</script>

<style lang='scss' scoped>
.body { margin-top: 14px; max-height: 60vh; overflow-y: auto; }
.empty { padding: 10px 0; font-size: 12px; color: var(--ft-text-3); }
.today { display: flex; gap: 22px; }
.stat { display: flex; flex-direction: column; }
.stat .num { font-size: 18px; color: var(--ft-text); }
.stat .cap { font-size: 10.5px; color: var(--ft-text-3); }
.chart { display: flex; align-items: flex-end; gap: 3px; height: 48px; margin-top: 14px; }
.bar { flex: 1; height: 100%; display: flex; align-items: flex-end; max-width: 14px; }
.bar .fill { width: 100%; border-radius: 1px; background: var(--ft-playing); opacity: 0.7; }
.section-label { margin: 16px 0 4px; font-size: 10.5px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--ft-text-3); }
.day { margin: 10px 0 2px; font-size: 11px; color: var(--ft-text-2); }
.line {
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 28px;
    padding: 0 8px;
    border-radius: 6px;
    font-size: 12.5px;
    color: var(--ft-text);
    &:hover { background: var(--ft-hover); }
    .text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .meta { flex-shrink: 0; font-size: 10.5px; color: var(--ft-text-3); }
}
</style>

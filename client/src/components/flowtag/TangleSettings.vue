<template>
<div class='tangle'>
    <div v-if='showToken' class='row-item'>
        <span class='row-label'>Access token</span>
        <span class='bind'>
            <Input v-model='token' type='password' autocomplete='off' placeholder='Paste token' class='token-input' @keydown.enter='connect' />
            <Button variant='default' :disabled='!token.trim() || connecting' @click='connect'>Save</Button>
        </span>
    </div>
    <div v-if='connected' class='row-item'>
        <span class='row-label'>
            Connected as {{ user }}
            <span class='sub'>{{ status?.matched }} of {{ status?.total }} tracks found in Tangle</span>
        </span>
        <Button @click='disconnect'>Disconnect</Button>
    </div>
    <div v-if='errorText' class='note err'>{{ errorText }}</div>
    <div v-if='previewing' class='note'>Checking Tangle…</div>
    <div v-if='revokeNote' class='note'>Also revoke the token in Tangle's dashboard.</div>

    <label class='row-item'>
        <span class='row-label'>Sync tags to Tangle</span>
        <Switch :model-value='$1t.settings.value.flowtag.tangleSync' :disabled='!connected || previewing' @update:model-value='toggle' />
    </label>

    <div v-if='status' class='row-item status'>
        <span class='row-label'>
            <span>{{ statusLine(status, now) }}</span>
            <span v-if='unmatched' class='sub unmatched' :title='status.unmatchedPaths.join("\n")'>{{ unmatched }}</span>
        </span>
    </div>
    <div v-for='[id, name] in status?.unlinked ?? []' :key='id' class='row-item'>
        <span class='row-label'>{{ name }}</span>
        <Button @click='$1t.send("tangleResendTag", { tagId: id })'>Send again</Button>
    </div>
</div>
</template>

<script lang='ts' setup>
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { confirmDialog } from '@/lib/dialogs';
import { connectedUser, connectErrorText, isConnected, needsToken, PREVIEW_TIMEOUT_MS, statusLine, unmatchedLine } from '@/lib/tangle';
import { get1t } from '../../scripts/onetagger';

const $1t = get1t();
const token = ref('');
const connecting = ref(false);
const revokeNote = ref(false);
const now = ref(Math.floor(Date.now() / 1000));
let timer: ReturnType<typeof setInterval> | undefined;

const status = computed(() => $1t.tangleStatus.value);
const user = computed(() => connectedUser(status.value, $1t.tangleUser.value));
const connected = computed(() => isConnected(status.value, $1t.tangleUser.value));
const showToken = computed(() => needsToken(status.value, $1t.tangleUser.value));
const previewing = ref(false);
const localError = ref('');
const unmatched = computed(() => (status.value ? unmatchedLine(status.value) : ''));
const errorText = computed(() => localError.value || ($1t.tangleError.value ? connectErrorText($1t.tangleError.value) : ''));

onMounted(() => {
    $1t.send('tangleStatus');
    timer = setInterval(() => { now.value = Math.floor(Date.now() / 1000); }, 15_000);
});
onUnmounted(() => clearInterval(timer));

// A reply (tangleConnected or tangleError) ends the connecting state; connect() clearing both does not
watch([() => $1t.tangleError.value, () => $1t.tangleUser.value], ([err, connectedAs]) => {
    if (err || connectedAs) connecting.value = false;
});

function connect() {
    const value = token.value.trim();
    if (!value) return;
    connecting.value = true;
    revokeNote.value = false;
    localError.value = '';
    $1t.tangleError.value = null;
    // Cleared so a reply naming the same user still registers as a change
    $1t.tangleUser.value = null;
    $1t.send('tangleConnect', { token: value });
    token.value = '';
}

function disconnect() {
    $1t.tangleUser.value = null;
    $1t.send('tangleDisconnect');
    setSync(false);
    revokeNote.value = true;
    $1t.send('tangleStatus');
}

function setSync(enabled: boolean) {
    $1t.settings.value.flowtag.tangleSync = enabled;
    $1t.saveSettings(false);
    $1t.send('tangleSetEnabled', { enabled });
}

type PreviewResult = { tags: number, entries: number } | { error: string };

/** Ask the backend how much enabling would create; ends early on a tangleError */
function requestPreview(): Promise<PreviewResult> {
    return new Promise((resolve) => {
        $1t.tanglePreview.value = null;
        $1t.tangleError.value = null;
        const finish = (r: PreviewResult) => { clearTimeout(timeout); stop(); resolve(r); };
        const stop = watch([$1t.tanglePreview, $1t.tangleError], ([p, err]) => {
            if (err) finish({ error: connectErrorText(err) });
            else if (p) finish(p);
        });
        const timeout = setTimeout(() => finish({ error: "Tangle didn't answer. Try again." }), PREVIEW_TIMEOUT_MS);
        $1t.send('tanglePreview');
    });
}

async function toggle(on: boolean) {
    if (!on) return setSync(false);
    if (previewing.value) return;
    localError.value = '';
    previewing.value = true;
    const preview = await requestPreview().finally(() => { previewing.value = false; });
    if ('error' in preview) {
        localError.value = preview.error;
        return;
    }
    const ok = await confirmDialog({
        title: 'Sync tags to Tangle',
        message: `This creates ${preview.tags} tags in Tangle and adds ${preview.entries} tracks to them.`,
        okLabel: 'Sync',
    });
    if (ok) setSync(true);
}
</script>

<style lang='scss' scoped>
.row-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 34px;
    border-bottom: 1px solid var(--ft-line);
    &:last-child { border-bottom: none; }
}
.row-label {
    font-size: 12.5px;
    color: var(--ft-text-2);
    .sub {
        font-size: 11px;
        color: var(--ft-text-3);
        margin-left: 6px;
    }
    .unmatched { cursor: help; }
}
.bind { display: inline-flex; align-items: center; gap: 8px; }
.token-input { width: 200px; }
.note {
    font-size: 11.5px;
    color: var(--ft-text-3);
    margin: 2px 0 4px;
    &.err { color: var(--ft-danger); }
}
</style>

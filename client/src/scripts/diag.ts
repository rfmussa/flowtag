import { get1t } from './onetagger';

/**
 * Flight recorder: structured breadcrumbs from the client, written into the
 * app's own log file (Settings folder / onetagger.log) through the clientLog
 * websocket action, interleaved with the Rust side's entries. One file tells
 * the whole story; Settings → Advanced → "Copy debug report" hands it over.
 *
 * Every entry is a single JSON line: { seq, win, event, ...data }.
 * Keep breadcrumbs low-volume: user intents, state transitions and errors —
 * never per-keystroke or per-frame chatter.
 */

let seq = 0;

function windowKind(): string {
    return window.location.hash.startsWith('#/mini') ? 'mini' : 'main';
}

function emit(level: 'info' | 'warn' | 'error', event: string, data?: Record<string, unknown>) {
    const entry = { seq: ++seq, win: windowKind(), event, ...data };
    let message: string;
    try {
        message = JSON.stringify(entry);
    } catch {
        message = JSON.stringify({ seq: entry.seq, win: entry.win, event, unserializable: true });
    }
    // Mirror to the devtools console for live debugging
    // eslint-disable-next-line no-console
    console.debug(`[diag] ${message}`);
    // send() awaits the ws connection, so early entries queue naturally;
    // after a dead socket this throws and the console mirror is all we keep
    try {
        get1t().send('clientLog', { level, message }).catch(() => { /* socket gone */ });
    } catch { /* store not ready */ }
}

export function diag(event: string, data?: Record<string, unknown>) {
    emit('info', event, data);
}

export function diagWarn(event: string, data?: Record<string, unknown>) {
    emit('warn', event, data);
}

export function diagError(event: string, data?: Record<string, unknown>) {
    emit('error', event, data);
}

const clip = (v: unknown, n = 1500) => String(v ?? '').slice(0, n);

/** Install global error hooks; called once per window (main and mini) */
export function initDiag() {
    window.addEventListener('error', (e) => {
        diagError('jsError', {
            message: clip(e.message, 400),
            source: `${e.filename?.split('/').pop() ?? '?'}:${e.lineno}:${e.colno}`,
            stack: clip((e.error as Error | undefined)?.stack),
        });
    });
    window.addEventListener('unhandledrejection', (e) => {
        diagError('unhandledRejection', {
            reason: clip(e.reason, 400),
            stack: clip((e.reason as Error | undefined)?.stack),
        });
    });
    diag('appBoot', {
        href: window.location.hash,
        ua: navigator.userAgent.includes('OneTagger') ? 'webview' : 'browser',
        screen: `${window.innerWidth}x${window.innerHeight}`,
    });
}

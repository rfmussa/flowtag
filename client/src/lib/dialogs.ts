import { computed, shallowRef } from 'vue';

/**
 * Programmatic confirm / prompt / alert dialogs (what Quasar's Dialog.create
 * did). Requests queue up and show one at a time; <DialogHost /> in App.vue
 * renders the head of the queue and settles it.
 */

export type DialogKind = 'confirm' | 'prompt' | 'alert';

export interface DialogOptions {
    title: string;
    message?: string;
    okLabel?: string;
    cancelLabel?: string;
    /** Red OK button: deletes and other data-losing actions */
    destructive?: boolean;
}

export interface PromptOptions extends DialogOptions {
    value?: string;
    placeholder?: string;
}

export interface DialogRequest {
    readonly id: number;
    readonly kind: DialogKind;
    readonly title: string;
    readonly message?: string;
    readonly okLabel: string;
    /** undefined = no cancel button (alerts) */
    readonly cancelLabel?: string;
    readonly destructive: boolean;
    readonly value?: string;
    readonly placeholder?: string;
}

/** OK carries the prompt's text (or true); cancel / dismiss is false */
export type DialogResult = string | boolean;

interface Pending {
    readonly request: DialogRequest;
    readonly resolve: (result: DialogResult) => void;
}

const queue = shallowRef<readonly Pending[]>([]);
let nextId = 1;

export const currentDialog = computed<DialogRequest | undefined>(() => queue.value[0]?.request);

function enqueue(kind: DialogKind, opts: PromptOptions): Promise<DialogResult> {
    return new Promise((resolve) => {
        const request: DialogRequest = {
            id: nextId++,
            kind,
            title: opts.title,
            message: opts.message,
            okLabel: opts.okLabel ?? 'OK',
            cancelLabel: kind === 'alert' ? undefined : (opts.cancelLabel ?? 'Cancel'),
            destructive: opts.destructive ?? false,
            value: kind === 'prompt' ? (opts.value ?? '') : undefined,
            placeholder: opts.placeholder,
        };
        queue.value = [...queue.value, { request, resolve }];
    });
}

/** Settle a shown request; stale or repeated settles are ignored */
export function settleDialog(id: number, result: DialogResult): void {
    const pending = queue.value.find((p) => p.request.id === id);
    if (!pending) return;
    queue.value = queue.value.filter((p) => p !== pending);
    pending.resolve(result);
}

export async function confirmDialog(opts: DialogOptions): Promise<boolean> {
    return (await enqueue('confirm', opts)) !== false;
}

/** The trimmed text, or undefined when cancelled or left blank */
export async function promptDialog(opts: PromptOptions): Promise<string | undefined> {
    const result = await enqueue('prompt', opts);
    if (typeof result !== 'string') return undefined;
    const text = result.trim();
    return text === '' ? undefined : text;
}

export async function alertDialog(opts: Omit<DialogOptions, 'cancelLabel'>): Promise<void> {
    await enqueue('alert', opts);
}

/** Tests only: drop every pending request */
export function resetDialogs(): void {
    queue.value = [];
}

import { beforeEach, describe, expect, it } from 'vitest';
import { alertDialog, confirmDialog, currentDialog, promptDialog, resetDialogs, settleDialog } from './dialogs';

beforeEach(() => resetDialogs());

describe('confirmDialog', () => {
    it('shows the request and resolves true when confirmed', async () => {
        const result = confirmDialog({ title: 'Delete tag', message: 'Sure?', okLabel: 'Delete', destructive: true });
        expect(currentDialog.value).toMatchObject({ kind: 'confirm', title: 'Delete tag', message: 'Sure?', okLabel: 'Delete', destructive: true });
        settleDialog(currentDialog.value!.id, true);
        await expect(result).resolves.toBe(true);
        expect(currentDialog.value).toBeUndefined();
    });

    it('resolves false when cancelled', async () => {
        const result = confirmDialog({ title: 'Remove folder?' });
        settleDialog(currentDialog.value!.id, false);
        await expect(result).resolves.toBe(false);
    });

    it('defaults the button labels', () => {
        confirmDialog({ title: 'X' });
        expect(currentDialog.value).toMatchObject({ okLabel: 'OK', cancelLabel: 'Cancel', destructive: false });
    });
});

describe('promptDialog', () => {
    it('resolves the trimmed text on OK', async () => {
        const result = promptDialog({ title: 'New playlist', value: '' });
        expect(currentDialog.value).toMatchObject({ kind: 'prompt', value: '' });
        settleDialog(currentDialog.value!.id, '  Peak time  ');
        await expect(result).resolves.toBe('Peak time');
    });

    it('resolves undefined on cancel or blank text', async () => {
        const cancelled = promptDialog({ title: 'Rename', value: 'Old' });
        settleDialog(currentDialog.value!.id, false);
        await expect(cancelled).resolves.toBeUndefined();

        const blank = promptDialog({ title: 'Rename', value: 'Old' });
        settleDialog(currentDialog.value!.id, '   ');
        await expect(blank).resolves.toBeUndefined();
    });
});

describe('alertDialog', () => {
    it('has no cancel button and resolves when dismissed', async () => {
        const result = alertDialog({ title: 'Error', message: 'boom' });
        expect(currentDialog.value).toMatchObject({ kind: 'alert', cancelLabel: undefined });
        settleDialog(currentDialog.value!.id, true);
        await expect(result).resolves.toBeUndefined();
    });
});

describe('queue', () => {
    it('shows requests one at a time, in order', async () => {
        const first = confirmDialog({ title: 'First' });
        const second = confirmDialog({ title: 'Second' });
        expect(currentDialog.value?.title).toBe('First');
        settleDialog(currentDialog.value!.id, true);
        expect(currentDialog.value?.title).toBe('Second');
        settleDialog(currentDialog.value!.id, false);
        await expect(first).resolves.toBe(true);
        await expect(second).resolves.toBe(false);
        expect(currentDialog.value).toBeUndefined();
    });

    it('ignores a stale or repeated settle', async () => {
        const first = confirmDialog({ title: 'First' });
        const id = currentDialog.value!.id;
        confirmDialog({ title: 'Second' });
        settleDialog(id, true);
        settleDialog(id, false);
        expect(currentDialog.value?.title).toBe('Second');
        await expect(first).resolves.toBe(true);
    });
});

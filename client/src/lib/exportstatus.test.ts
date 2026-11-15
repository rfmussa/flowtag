import { describe, expect, it, vi } from 'vitest';
import { TagStore } from '@/scripts/tagstore';

vi.mock('@/scripts/onetagger', () => ({ get1t: () => ({ send: vi.fn() }) }));
vi.mock('@/scripts/diag', () => ({ diag: vi.fn(), diagError: vi.fn() }));

describe('rekordbox export progress', () => {
    it('starts_running_on_the_first_progress_message', () => {
        const s = new TagStore();
        expect(s.exportStatus.value).toEqual({ running: false, done: 0, total: 0 });
        expect(s.handleMessage({ action: 'rekordboxExportProgress', done: 10, total: 7678 })).toBe(true);
        expect(s.exportStatus.value).toEqual({ running: true, done: 10, total: 7678 });
    });

    it('updates_counts_as_progress_arrives', () => {
        const s = new TagStore();
        s.handleMessage({ action: 'rekordboxExportProgress', done: 10, total: 7678 });
        s.handleMessage({ action: 'rekordboxExportProgress', done: 500, total: 7678 });
        expect(s.exportStatus.value).toEqual({ running: true, done: 500, total: 7678 });
    });

    it('clears_on_successful_export_done', () => {
        const s = new TagStore();
        s.handleMessage({ action: 'rekordboxExportProgress', done: 7678, total: 7678 });
        s.handleMessage({ action: 'rekordboxExportDone', result: { written: 7000, unchanged: 678 } });
        expect(s.exportStatus.value).toEqual({ running: false, done: 0, total: 0 });
    });

    it('holds_a_persistent_result_when_files_failed', () => {
        const s = new TagStore();
        s.handleMessage({ action: 'rekordboxExportProgress', done: 7678, total: 7678 });
        s.handleMessage({ action: 'rekordboxExportDone', result: { written: 7600, unchanged: 50, failed: 28 } });
        expect(s.exportStatus.value).toEqual({
            running: false, done: 0, total: 0,
            result: { written: 7600, unchanged: 50, failed: 28 },
        });
    });
});

import { describe, expect, it } from 'vitest';
import { exportDone, exportProgress, IDLE_EXPORT } from '@/lib/exportstatus';

describe('rekordbox export status', () => {
    it('starts_running_on_the_first_progress_message', () => {
        expect(exportProgress(IDLE_EXPORT, { done: 10, total: 7678 })).toEqual({ running: true, done: 10, total: 7678 });
    });

    it('updates_counts_as_progress_arrives', () => {
        const first = exportProgress(IDLE_EXPORT, { done: 10, total: 7678 });
        expect(exportProgress(first, { done: 500, total: 7678 })).toEqual({ running: true, done: 500, total: 7678 });
    });

    it('missing_counts_default_to_zero', () => {
        expect(exportProgress(IDLE_EXPORT, {})).toEqual({ running: true, done: 0, total: 0 });
    });

    it('clears_on_successful_export_done', () => {
        const running = exportProgress(IDLE_EXPORT, { done: 7678, total: 7678 });
        expect(exportDone(running, { result: { written: 7000, unchanged: 678 } })).toEqual(IDLE_EXPORT);
    });

    it('holds_a_persistent_result_when_files_failed', () => {
        const running = exportProgress(IDLE_EXPORT, { done: 7678, total: 7678 });
        expect(exportDone(running, { result: { written: 7600, unchanged: 50, failed: 28 } })).toEqual({
            running: false, done: 0, total: 0,
            result: { written: 7600, unchanged: 50, failed: 28 },
        });
    });

    it('never_mutates_the_previous_state', () => {
        const before = { running: true, done: 1, total: 2 };
        exportDone(before, { result: { failed: 1 } });
        exportProgress(before, { done: 2, total: 2 });
        expect(before).toEqual({ running: true, done: 1, total: 2 });
    });
});

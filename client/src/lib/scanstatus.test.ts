import { describe, expect, it } from 'vitest';
import { IDLE_SCAN, scanDone, scanProgress } from '@/lib/scanstatus';

describe('scan status', () => {
    it('progress_marks_running_with_counts', () => {
        expect(scanProgress(IDLE_SCAN, { done: 3, total: 9 })).toEqual({ running: true, done: 3, total: 9 });
    });

    it('done_resets_counts_and_keeps_the_result', () => {
        const result = { added: 1, updated: 2, removed: 3, failed: 0 };
        const running = scanProgress(IDLE_SCAN, { done: 9, total: 9 });
        expect(scanDone(running, result)).toEqual({ running: false, done: 0, total: 0, result });
    });

    it('never_mutates_the_previous_state', () => {
        const before = { running: false, done: 0, total: 0 };
        scanProgress(before, { done: 1, total: 2 });
        expect(before).toEqual({ running: false, done: 0, total: 0 });
    });
});

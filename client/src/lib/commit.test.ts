import { describe, expect, it } from 'vitest';
import { commitPayload, initialTicks, toggleTick } from '@/lib/commit';

const rows = [
    { trackId: 1, skip: false, ready: true },
    { trackId: 2, skip: false, ready: false },
    { trackId: 3, skip: true, ready: true },
];

describe('commit ticks', () => {
    it('ticked_defaults_ready_only', () => {
        expect(initialTicks(rows)).toEqual({ 1: true, 2: false, 3: true });
    });

    it('toggle_returns_a_new_map', () => {
        const before = initialTicks(rows);
        const after = toggleTick(before, 2);
        expect(after[2]).toBe(true);
        expect(before[2]).toBe(false);
    });

    it('commit_payload_is_ticked_ids_and_trimmed_message', () => {
        const ticks = toggleTick(initialTicks(rows), 2);
        expect(commitPayload(rows, ticks, '  evening crate  ')).toEqual({ trackIds: [1, 2, 3], message: 'evening crate' });
        expect(commitPayload(rows, { 1: false, 2: false, 3: true }, '')).toEqual({ trackIds: [3], message: '' });
    });
});

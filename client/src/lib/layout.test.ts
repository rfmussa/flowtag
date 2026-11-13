import { describe, expect, it } from 'vitest';
import { capChips, waveformWaves } from './layout';

describe('waveformWaves', () => {
    it('scales_with_width_between_its_clamps', () => {
        expect(waveformWaves(1280)).toBe(28);
        expect(waveformWaves(1700)).toBe(70);
        expect(waveformWaves(3000)).toBe(70);
    });

    it('never_goes_below_the_floor_on_narrow_windows', () => {
        // The old formula went negative below 1200 and broke the waveform
        expect(waveformWaves(1200)).toBe(20);
        expect(waveformWaves(900)).toBe(14);
        expect(waveformWaves(800)).toBe(14);
    });
});

describe('capChips', () => {
    const tags = ['a', 'b', 'c', 'd'];

    it('uncapped_shows_everything', () => {
        expect(capChips(tags)).toEqual({ shown: tags, more: 0 });
    });

    it('capped_shows_the_first_and_counts_the_rest', () => {
        expect(capChips(tags, 1)).toEqual({ shown: ['a'], more: 3 });
        expect(capChips(tags, 4)).toEqual({ shown: tags, more: 0 });
        expect(capChips([], 1)).toEqual({ shown: [], more: 0 });
    });
});

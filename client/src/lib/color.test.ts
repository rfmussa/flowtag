import { describe, expect, it } from 'vitest';
import { TAG_COLORS, parseHexColor } from './color';

describe('parseHexColor', () => {
    it('accepts #rrggbb and lowercases it', () => {
        expect(parseHexColor('#E5484D')).toBe('#e5484d');
    });

    it('accepts a missing # and surrounding whitespace', () => {
        expect(parseHexColor('  30a46c ')).toBe('#30a46c');
    });

    it('expands #rgb shorthand', () => {
        expect(parseHexColor('#f0a')).toBe('#ff00aa');
    });

    it('rejects anything else', () => {
        for (const bad of ['', '#', '#12', '#12345', '#1234567', 'red', '#ggggggg', '#zzzzzz', 'rgb(1,2,3)']) {
            expect(parseHexColor(bad), bad).toBeUndefined();
        }
    });
});

describe('TAG_COLORS', () => {
    it('mirrors the backend palette: 12 valid colors, no duplicates', () => {
        expect(TAG_COLORS).toHaveLength(12);
        expect(new Set(TAG_COLORS).size).toBe(12);
        for (const c of TAG_COLORS) expect(parseHexColor(c)).toBe(c);
    });
});

import { describe, expect, it } from 'vitest';
import { shouldPreload } from './player';

describe('shouldPreload', () => {
    it('preload_skips_null_and_playing_path', () => {
        expect(shouldPreload(null, null)).toBe(false);
        expect(shouldPreload(null, '/m/a.mp3')).toBe(false);
        expect(shouldPreload('/m/a.mp3', '/m/a.mp3')).toBe(false);
        expect(shouldPreload('/m/a.mp3', '/m/b.mp3')).toBe(true);
        expect(shouldPreload('/m/a.mp3', null)).toBe(true);
    });
});

import { describe, expect, it } from 'vitest';
import { stripFrameHelper, validateFrameName } from './tagframe';

describe('validateFrameName', () => {
    it('accepts a normal uppercase frame for each format', () => {
        expect(validateFrameName('id3', 'TCON')).toBeUndefined();
        expect(validateFrameName('vorbis', 'GENRE')).toBeUndefined();
        expect(validateFrameName('mp4', '©gen')).toBeUndefined();
    });

    it('rejects an empty name', () => {
        expect(validateFrameName('id3', '')).toBe("Shouldn't be empty");
    });

    it('wants uppercase, except MP4 atoms and ID3 custom (non 4-char) names', () => {
        expect(validateFrameName('vorbis', 'Genre')).toBe('Tag names should be uppercase');
        expect(validateFrameName('id3', 'Tcon')).toBe('Tag names should be uppercase');
        expect(validateFrameName('id3', 'My Custom')).toBeUndefined();
        expect(validateFrameName('mp4', 'genre')).toBeUndefined();
    });

    it('rejects spaces, except ID3 custom (non 4-char) names', () => {
        expect(validateFrameName('vorbis', 'MY TAG')).toBe("Tag names shouldn't contain spaces");
        expect(validateFrameName('id3', 'MY TAG')).toBeUndefined();
    });

    it('flags reserved frames, case-insensitively, with the tag editor list when asked', () => {
        expect(validateFrameName('id3', 'APIC')).toBe('APIC is reserved and might corrupt the file');
        expect(validateFrameName('vorbis', 'bpm')).toBe('BPM is reserved and might corrupt the file');
        expect(validateFrameName('id3', 'TBPM', true)).toBeUndefined();
        expect(validateFrameName('id3', 'COMM', true)).toBe('COMM is reserved and might corrupt the file');
    });
});

describe('stripFrameHelper', () => {
    it('drops the "(description)" suggestion suffix from ID3 / MP4 picks', () => {
        expect(stripFrameHelper('id3', 'TCON (Genre)')).toBe('TCON');
        expect(stripFrameHelper('mp4', '©gen (Genre)')).toBe('©gen');
    });

    it('leaves Vorbis names and plain names alone', () => {
        expect(stripFrameHelper('vorbis', 'GENRE (x)')).toBe('GENRE (x)');
        expect(stripFrameHelper('id3', 'TCON')).toBe('TCON');
    });
});

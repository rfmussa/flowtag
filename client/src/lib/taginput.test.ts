import { describe, expect, it } from 'vitest';
import { buildSuggestions, type SuggestionPool } from './taginput';

function tag(id: number, name: string, count = 1) {
    return { id, name, color: '#fff', count };
}

function pool(overrides: Partial<SuggestionPool> = {}): SuggestionPool {
    return {
        query: '',
        matches: [],
        appliedIds: new Set<number>(),
        exactTag: undefined,
        ...overrides,
    };
}

describe('buildSuggestions', () => {
    it('filters applied tags out of the matches', () => {
        const out = buildSuggestions(pool({
            query: 'd',
            matches: [tag(1, 'deep'), tag(2, 'dnb')],
            appliedIds: new Set([1]),
        }));
        expect(out.filter((s) => s.id !== undefined).map((s) => s.name)).toEqual(['dnb']);
    });

    it('caps the match list at 8', () => {
        const matches = Array.from({ length: 12 }, (_, i) => tag(i + 1, `t${i + 1}`));
        const out = buildSuggestions(pool({ query: 't', matches, exactTag: undefined }));
        // 8 matches + the create row
        expect(out.filter((s) => !s.remove && s.id !== undefined)).toHaveLength(8);
    });

    it('appends a create row when the text matches no tag exactly', () => {
        const out = buildSuggestions(pool({ query: 'deep h', matches: [tag(1, 'deep house')] }));
        const last = out[out.length - 1];
        expect(last.id).toBeUndefined();
        expect(last.name).toBe('deep h');
    });

    it('offers no create row when an exact tag exists', () => {
        const deep = tag(1, 'deep');
        const out = buildSuggestions(pool({ query: 'deep', matches: [deep], exactTag: deep }));
        expect(out.every((s) => s.id !== undefined)).toBe(true);
    });

    it('prepends a remove row when the exact match is already applied', () => {
        const deep = tag(1, 'deep');
        const out = buildSuggestions(pool({
            query: 'deep',
            matches: [deep, tag(2, 'deep house')],
            appliedIds: new Set([1]),
            exactTag: deep,
        }));
        expect(out[0]).toMatchObject({ id: 1, name: 'deep', remove: true });
        // The applied tag stays out of the plain matches
        expect(out.filter((s) => !s.remove).map((s) => s.name)).toEqual(['deep house']);
    });

    it('offers no remove row when the exact match is not applied', () => {
        const deep = tag(1, 'deep');
        const out = buildSuggestions(pool({ query: 'deep', matches: [deep], exactTag: deep }));
        expect(out.some((s) => s.remove)).toBe(false);
    });

    it('offers neither create nor remove rows on an empty query', () => {
        const out = buildSuggestions(pool({
            query: '',
            matches: [tag(1, 'deep')],
            appliedIds: new Set([2]),
        }));
        expect(out.every((s) => s.id !== undefined && !s.remove)).toBe(true);
    });
});

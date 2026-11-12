/** Suggestion-list building for the tag input — pure, so it's testable
 * without mounting the component. */

const MAX_MATCHES = 8;
const CREATE_ROW_COLOR = 'var(--ft-text-3)';

export interface TagSuggestion {
    /** Undefined = the "Create" row */
    id?: number;
    name: string;
    color: string;
    count?: number;
    /** True = the "Remove" row: the typed text exactly names an applied tag */
    remove?: boolean;
}

export interface SuggestionPool {
    /** Trimmed input text */
    query: string;
    /** Ranked store.suggest() output for the query */
    matches: { id: number; name: string; color: string; count?: number }[];
    /** Tag ids already on the target track */
    appliedIds: Set<number>;
    /** store.tagByName(query) — the exact-name match, applied or not */
    exactTag?: { id: number; name: string; color: string };
}

export function buildSuggestions(pool: SuggestionPool): TagSuggestion[] {
    const { query, matches, appliedIds, exactTag } = pool;
    const out: TagSuggestion[] = [];

    // Typing the exact name of an applied tag offers to remove it — the only
    // keyboard path to untag something that isn't the last chip
    if (query && exactTag && appliedIds.has(exactTag.id)) {
        out.push({ id: exactTag.id, name: exactTag.name, color: exactTag.color, remove: true });
    }

    for (const m of matches) {
        if (appliedIds.has(m.id)) continue;
        out.push({ ...m });
        if (out.length >= MAX_MATCHES + (out[0]?.remove ? 1 : 0)) break;
    }

    if (query && !exactTag) {
        out.push({ name: query, color: CREATE_ROW_COLOR });
    }
    return out;
}

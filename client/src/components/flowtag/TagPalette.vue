<template>
<div class='tag-palette overflow-auto'>
    <div v-if='sections.length === 0' class='palette-empty'>
        No tags yet. Press <span class='ft-kbd'>T</span> and type to create one.
    </div>

    <div v-for='section in sections' :key='section.key' class='section'>
        <div class='section-header'>
            <span class='section-dot' :style='{ backgroundColor: section.color }'></span>
            <span class='section-name'>{{ section.name }}</span>
        </div>
        <button
            v-for='tag in section.tags'
            :key='tag.id'
            type='button'
            class='pal-row'
            :class='{ active: activeIds.has(tag.id), disabled: !hasSelection && !filterIds.has(tag.id), filtering: filterIds.has(tag.id) }'
            :title='`${tag.name} — click: toggle on track · ⌘click: filter list`'
            @mousedown.prevent
            @click='toggle(tag, $event)'
        >
            <span class='pal-dot' :style='{ background: tag.color }'></span>
            <span class='pal-name'>{{ tag.name }}</span>
            <span v-if='keyLabel(tag)' class='ft-kbd pal-key'>{{ keyLabel(tag) }}</span>
            <span class='pal-count ft-num'>{{ tag.count }}</span>
        </button>
    </div>

    <div class='palette-hint'><span class='ft-kbd'>⌘</span> click a tag to filter the list</div>
</div>
</template>

<script lang='ts' setup>
import { computed } from 'vue';
import { FlowTag, getTagStore } from '../../scripts/tagstore';

const UNGROUPED_COLOR = 'var(--ft-text-3)';

interface Section { key: string; name: string; color: string; tags: FlowTag[] }

const store = getTagStore();

const hasSelection = computed(() => store.selectedId.value !== undefined);
const filterIds = computed<Set<number>>(() => new Set(store.filterTagIds.value));

// Tags on the selected track, for the highlighted state
const activeIds = computed<Set<number>>(() => {
    store.tracks.value; // reactive dep
    return new Set(store.selectedTrack()?.tagIds ?? []);
});

const byName = (a: FlowTag, b: FlowTag) => a.name.localeCompare(b.name);

const sections = computed<Section[]>(() => {
    const groups = [...store.groups.value].sort((a, b) => a.position - b.position || a.id - b.id);
    const groupIds = new Set(groups.map((g) => g.id));
    const out: Section[] = groups.map((g) => ({
        key: `g${g.id}`,
        name: g.name,
        color: g.color,
        tags: store.tags.value.filter((t) => t.groupId === g.id).sort(byName),
    }));
    const loose = store.tags.value.filter((t) => t.groupId === undefined || t.groupId === null || !groupIds.has(t.groupId));
    out.push({ key: 'none', name: 'Tags', color: UNGROUPED_COLOR, tags: loose.sort(byName) });
    return out.filter((s) => s.tags.length > 0);
});

/** Compact hotkey label, e.g. "⌃⇧A" */
function keyLabel(tag: FlowTag): string {
    const kb = tag.keybind;
    if (!kb || !kb.key) return '';
    return `${kb.ctrl ? '⌃' : ''}${kb.alt ? '⌥' : ''}${kb.shift ? '⇧' : ''}${String(kb.key).toUpperCase()}`;
}

function toggle(tag: FlowTag, e: MouseEvent) {
    (e.currentTarget as HTMLElement | null)?.blur();
    // Cmd/Ctrl+click filters the list by this tag instead of tagging
    if (e.metaKey || e.ctrlKey) {
        store.toggleTagFilter(tag.id);
        return;
    }
    if (store.selectedId.value === undefined) return;
    store.toggleTag(store.selectedId.value, tag);
}
</script>

<style lang='scss'>
.tag-palette {
    height: 100%;
    padding: 10px 8px;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;

    .palette-empty {
        font-size: 12px;
        color: var(--ft-text-3);
        padding: 8px;
        line-height: 1.6;
    }

    .section { margin-bottom: 14px; }
    .section-header {
        display: flex;
        align-items: center;
        gap: 7px;
        padding: 0 8px 5px;
        .section-dot { width: 7px; height: 7px; border-radius: 50%; flex-shrink: 0; }
        .section-name {
            font-size: 10.5px;
            font-weight: 600;
            letter-spacing: 0.08em;
            text-transform: uppercase;
            color: var(--ft-text-3);
        }
    }

    .pal-row {
        display: flex;
        align-items: center;
        gap: 8px;
        width: 100%;
        box-sizing: border-box;
        height: 26px;
        padding: 0 8px;
        border: none;
        background: transparent;
        border-radius: 5px;
        cursor: pointer;
        font-family: inherit;
        text-align: left;

        .pal-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
        .pal-name {
            flex: 1;
            min-width: 0;
            font-size: 12.5px;
            font-weight: 500;
            color: var(--ft-text-2);
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }
        .pal-key { flex-shrink: 0; }
        .pal-count { font-size: 10.5px; color: var(--ft-text-3); flex-shrink: 0; }

        &:hover { background: var(--ft-hover); .pal-name { color: var(--ft-text); } }
        &.active {
            background: var(--ft-active);
            .pal-name { color: var(--ft-text); font-weight: 600; }
        }
        &.filtering {
            background: var(--ft-raised);
            box-shadow: inset 0 0 0 1px var(--ft-line-strong);
            .pal-name { color: var(--ft-text); }
            .pal-dot { box-shadow: 0 0 0 2px var(--ft-bg), 0 0 0 3px var(--ft-text-3); }
        }
        &.disabled { opacity: 0.45; cursor: default; }
    }

    .palette-hint {
        margin-top: auto;
        padding: 8px;
        font-size: 11px;
        color: var(--ft-text-3);
        display: flex;
        align-items: center;
        gap: 5px;
    }
}
</style>

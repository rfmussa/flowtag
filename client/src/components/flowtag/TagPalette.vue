<template>
<div class='tag-palette scroll'>
    <div v-if='sections.length === 0' class='text-grey-6 text-caption q-pa-sm'>
        No tags yet. Type a tag to create one.
    </div>

    <div v-for='section in sections' :key='section.key' class='section'>
        <div class='section-header row items-center no-wrap'>
            <span class='dot' :style='{ backgroundColor: section.color }'></span>
            <span class='ellipsis'>{{ section.name }}</span>
        </div>
        <div class='chips'>
            <button
                v-for='tag in section.tags'
                :key='tag.id'
                type='button'
                class='pal-chip'
                :class='{ active: activeIds.has(tag.id), disabled: !hasSelection }'
                :style='{ backgroundColor: tag.color, color: chipTextColor(tag.color) }'
                :title='hasSelection ? tag.name : "Select a track first"'
                @mousedown.prevent
                @click='toggle(tag, $event)'
            >
                <span class='name ellipsis'>{{ tag.name }}</span>
                <span v-if='keyLabel(tag)' class='key'>{{ keyLabel(tag) }}</span>
                <span class='count'>{{ tag.count }}</span>
            </button>
        </div>
    </div>
</div>
</template>

<script lang='ts' setup>
import { computed } from 'vue';
import { FlowTag, getTagStore } from '../../scripts/tagstore';
import { chipTextColor } from './TagChips.vue';

const UNGROUPED_COLOR = '#757575';

interface Section { key: string; name: string; color: string; tags: FlowTag[] }

const store = getTagStore();

const hasSelection = computed(() => store.selectedId.value !== undefined);

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
    if (store.selectedId.value === undefined) return;
    store.toggleTag(store.selectedId.value, tag);
}
</script>

<style lang='scss'>
.tag-palette {
    height: 100%;
    padding: 6px 8px;

    .section { margin-bottom: 8px; }
    .section-header {
        font-size: 11px;
        text-transform: uppercase;
        letter-spacing: 0.04em;
        opacity: 0.75;
        margin-bottom: 3px;
        .dot { width: 8px; height: 8px; border-radius: 50%; margin-right: 6px; flex-shrink: 0; }
    }
    .chips { display: flex; flex-wrap: wrap; gap: 3px; }
    .pal-chip {
        display: inline-flex;
        align-items: center;
        max-width: 100%;
        border: 1px solid transparent;
        border-radius: 4px;
        padding: 1px 5px;
        font-size: 11px;
        line-height: 18px;
        cursor: pointer;
        outline: none;
        .name { min-width: 0; }
        .key {
            margin-left: 4px;
            padding: 0 3px;
            border-radius: 3px;
            font-size: 9px;
            font-weight: bold;
            background: rgba(0, 0, 0, 0.3);
            color: #fff;
        }
        .count { margin-left: 4px; font-size: 9px; opacity: 0.7; }
        &:hover:not(.disabled) { filter: brightness(1.15); }
        &.active { border-color: #fff; box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.5); }
        &.disabled { opacity: 0.4; cursor: default; }
    }
}
</style>

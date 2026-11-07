<template>
<div class='tag-input-wrap'>
    <input
        ref='inputEl'
        v-model='text'
        class='tag-input'
        type='text'
        placeholder='Type to tag… (Enter adds, Esc back)'
        spellcheck='false'
        autocomplete='off'
        @keydown='onKey'
        @focus='open = true'
        @blur='onBlur'
    />
    <!-- Suggestions -->
    <div v-if='open && trackId !== undefined && suggestions.length > 0' class='suggestions'>
        <div
            v-for='(tag, i) in suggestions'
            :key='tag.id ?? "new"'
            class='suggestion row items-center q-px-sm'
            :class='{highlighted: i === highlighted}'
            @mousedown.prevent='apply(i)'
            @mousemove='highlighted = i'
        >
            <span class='dot q-mr-sm' :style='{background: tag.color}'></span>
            <span class='name ellipsis'>{{ tag.id === undefined ? `Create "${tag.name}"` : tag.name }}</span>
            <span v-if='tag.id !== undefined' class='count text-grey-6 q-ml-sm'>{{ tag.count }}</span>
        </div>
    </div>
</div>
</template>

<script lang='ts' setup>
import { computed, ref, watch } from 'vue';
import { getTagStore } from '../../scripts/tagstore';

const props = defineProps<{ trackId?: number }>();
const store = getTagStore();

const inputEl = ref<HTMLInputElement>();
const text = ref('');
const highlighted = ref(0);
const open = ref(false);

interface Suggestion { id?: number; name: string; color: string; count?: number }

// Pool matches minus tags the track already carries, plus a "create" row
// when the text matches nothing exactly
const suggestions = computed<Suggestion[]>(() => {
    const q = text.value.trim();
    const track = store.selectedTrack();
    const applied = track && track.id === props.trackId ? new Set(track.tagIds) : new Set<number>();
    const matches: Suggestion[] = store
        .suggest(q, 8 + applied.size)
        .filter((t) => !applied.has(t.id))
        .slice(0, 8);
    if (q && !store.tagByName(q)) {
        matches.push({ name: q, color: '#555555' });
    }
    return matches;
});

// Keep highlight valid when the list shrinks
watch(suggestions, (s) => {
    if (highlighted.value >= s.length) highlighted.value = Math.max(s.length - 1, 0);
});
watch(text, () => { highlighted.value = 0; });

// Focus requests from the keyboard layer (T / Tab)
watch(store.focusInputSignal, () => {
    inputEl.value?.focus();
    open.value = true;
});

function apply(index: number) {
    if (props.trackId === undefined) return;
    const s = suggestions.value[index];
    if (!s) return;
    store.addTag(props.trackId, s.name);
    text.value = '';
    highlighted.value = 0;
    inputEl.value?.focus();
}

function onKey(e: KeyboardEvent) {
    switch (e.code) {
        case 'ArrowDown':
            highlighted.value = Math.min(highlighted.value + 1, suggestions.value.length - 1);
            e.preventDefault();
            break;
        case 'ArrowUp':
            highlighted.value = Math.max(highlighted.value - 1, 0);
            e.preventDefault();
            break;
        case 'Enter':
            if (text.value.trim() || suggestions.value.length > 0) apply(highlighted.value);
            e.preventDefault();
            break;
        case 'Backspace':
            // Empty input: remove the last chip
            if (!text.value && props.trackId !== undefined) {
                const track = store.selectedTrack();
                const last = track?.tagIds[track.tagIds.length - 1];
                if (last !== undefined) store.removeTag(props.trackId, last);
                e.preventDefault();
            }
            break;
        case 'Escape':
        case 'Tab':
            inputEl.value?.blur();
            e.preventDefault();
            break;
    }
    // Keep every other key inside the input (the global handler must not see it)
    e.stopPropagation();
}

function onBlur() {
    // Delay so a suggestion mousedown can land first
    setTimeout(() => { open.value = false; }, 100);
}
</script>

<style lang='scss' scoped>
.tag-input-wrap {
    position: relative;
    flex: 1;
}
.tag-input {
    width: 100%;
    box-sizing: border-box;
    background: var(--ft-panel);
    border: 1px solid var(--ft-line);
    border-radius: 6px;
    color: var(--ft-text);
    padding: 7px 12px;
    font-size: 13px;
    font-family: inherit;
    outline: none;
    transition: border-color 80ms ease-out;

    &::placeholder { color: var(--ft-text-3); }
    &:hover { border-color: var(--ft-line-strong); }
    &:focus { border-color: var(--ft-focus); }
}
.suggestions {
    position: absolute;
    bottom: calc(100% + 6px);
    left: 0;
    min-width: 260px;
    max-width: 100%;
    background: var(--ft-overlay);
    border: 1px solid var(--ft-line-strong);
    border-radius: 8px;
    overflow: hidden;
    z-index: 100;
    padding: 4px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45), 0 2px 6px rgba(0, 0, 0, 0.3);
}
.suggestion {
    height: 28px;
    cursor: pointer;
    font-size: 12.5px;
    border-radius: 5px;
    color: var(--ft-text-2);

    &.highlighted {
        background: var(--ft-active);
        color: var(--ft-text);
    }
    .dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        flex-shrink: 0;
    }
    .name {
        flex: 1;
        min-width: 0;
        font-weight: 500;
    }
    .count {
        font-size: 10.5px;
        font-variant-numeric: tabular-nums;
        color: var(--ft-text-3);
    }
}
</style>

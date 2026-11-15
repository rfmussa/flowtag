<template>
<!-- Audition themes the way you audition tracks: focus or hover previews the
     whole app live behind the dialog, Enter or click commits, Esc or leaving
     reverts to the committed theme. -->
<div class='theme-picker' role='radiogroup' aria-label='Theme'>
    <button
        v-for='(t, i) in THEMES'
        :key='t.id'
        ref='rowEls'
        type='button'
        class='theme-row'
        role='radio'
        :aria-checked='committed === t.id'
        :class='{ on: committed === t.id }'
        :tabindex='t.id === committed ? 0 : -1'
        @focus='preview(t.id)'
        @mouseenter='preview(t.id)'
        @mouseleave='revert'
        @blur='revert'
        @click='commit(t.id)'
        @keydown.enter.prevent='commit(t.id)'
        @keydown.escape.prevent='revert'
        @keydown.down.prevent='focusRow(i + 1)'
        @keydown.right.prevent='focusRow(i + 1)'
        @keydown.up.prevent='focusRow(i - 1)'
        @keydown.left.prevent='focusRow(i - 1)'
    >
        <span class='theme-swatch' aria-hidden='true'>
            <i v-for='c in t.swatch' :key='c' :style='{ background: c }'></i>
        </span>
        <span class='theme-name'>{{ t.name }}</span>
        <span class='theme-hint'>{{ t.hint }}</span>
        <CheckIcon v-if='committed === t.id' class='theme-check size-3.5' />
    </button>
</div>
</template>

<script lang='ts' setup>
import { computed, ref } from 'vue';
import { CheckIcon } from '@lucide/vue';
import { get1t } from '../../scripts/onetagger';
import { applyTheme, DEFAULT_THEME, THEMES, ThemeId } from '../../scripts/themes';

const $1t = get1t();
const rowEls = ref<HTMLButtonElement[]>([]);

const committed = computed(() => $1t.settings.value.flowtag.theme ?? DEFAULT_THEME);

/** Live audition: apply without saving */
function preview(id: ThemeId) {
    applyTheme(id, true);
}

/** Leaving a row restores the committed theme */
function revert() {
    applyTheme(committed.value, true);
}

function commit(id: ThemeId) {
    $1t.settings.value.flowtag.theme = id;
    $1t.saveSettings(false);
    applyTheme(id, true);
}

function focusRow(index: number) {
    const rows = rowEls.value;
    if (!rows.length) return;
    const next = Math.min(Math.max(index, 0), rows.length - 1);
    rows[next]?.focus();
}
</script>

<style lang='scss' scoped>
.theme-picker {
    display: flex;
    flex-direction: column;
    gap: 2px;
}
.theme-row {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 10px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: var(--ft-text-2);
    font-size: 12.5px;
    font-weight: 500;
    text-align: left;
    cursor: default;
    transition: background 80ms ease-out, color 80ms ease-out, border-color 80ms ease-out;

    &:hover { background: var(--ft-hover); color: var(--ft-text); }
    &.on {
        background: var(--ft-raised);
        border-color: var(--ft-line-strong);
        color: var(--ft-text);
    }
}
.theme-swatch {
    display: inline-flex;
    gap: 3px;

    i {
        width: 12px;
        height: 12px;
        border-radius: 3px;
        border: 1px solid var(--ft-line-strong);
    }
}
.theme-name { min-width: 72px; }
.theme-hint { color: var(--ft-text-3); font-size: 11.5px; }
.theme-check { margin-left: auto; color: var(--ft-text-2); }
</style>

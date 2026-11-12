import { ref } from 'vue';
import { notify } from '@/lib/notify';
import { getTagStore } from './tagstore';

/** Whether the commit sheet is showing; <CommitSheet /> in App.vue binds to it */
export const commitSheetOpen = ref(false);

/** Opens the stage commit sheet (⌘Enter, Stage header, quit flow). An empty stage only toasts. */
export function openCommitSheet(): void {
    if (getTagStore().stageTotal.value === 0) {
        notify({ message: 'Stage is empty', timeout: 1500 });
        return;
    }
    commitSheetOpen.value = true;
}

/** ⌘Enter / Ctrl+Enter: opens the commit sheet even from a focused input */
export function isCommitChord(event: KeyboardEvent): boolean {
    return event.code === 'Enter' && (event.metaKey || event.ctrlKey) && !event.shiftKey && !event.altKey;
}

/** Does the tag input keep this key to itself? The commit chord passes through
 *  untouched (no add-tag, no stopPropagation) so the global handler sees it. */
export function tagInputOwnsKey(event: KeyboardEvent): boolean {
    return !isCommitChord(event);
}

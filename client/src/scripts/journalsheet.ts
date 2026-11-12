import { ref } from 'vue';
import { getTagStore } from './tagstore';

/** Whether the Journal dialog is showing; <JournalDialog /> in App.vue binds to it */
export const journalOpen = ref(false);

/** Opens the Journal and fetches a fresh copy */
export function openJournal(): void {
    journalOpen.value = true;
    getTagStore().requestJournal();
}

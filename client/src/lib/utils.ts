import type { ClassValue } from 'clsx';
import { clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

/** shadcn-vue's class combiner: conditional classes, Tailwind conflicts resolved */
export function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

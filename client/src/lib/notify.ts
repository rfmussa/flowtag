import { toast } from 'vue-sonner';

/**
 * Toasts via vue-sonner (shadcn's toast). Accepts the option shape the app's
 * former Quasar Notify calls used, so call sites only change their import.
 * `position` and `progress` are accepted and ignored: the Toaster mounted in
 * App.vue owns placement.
 */
export interface NotifyOptions {
    message: string;
    /** Quasar-style severity names */
    type?: 'positive' | 'negative' | 'warning' | 'info';
    /** Older call sites pass severity as a color */
    color?: string;
    /** Milliseconds on screen */
    timeout?: number;
    position?: string;
    progress?: boolean;
}

export function notify(options: NotifyOptions) {
    const severity = options.type ?? options.color;
    const opts = options.timeout ? { duration: options.timeout } : {};
    switch (severity) {
        case 'positive': return toast.success(options.message, opts);
        case 'negative': return toast.error(options.message, opts);
        case 'warning': return toast.warning(options.message, opts);
        case 'info': return toast.info(options.message, opts);
        default: return toast(options.message, opts);
    }
}

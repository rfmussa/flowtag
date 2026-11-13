/** FlowTag themes: each is a value set for the --ft-* tokens, applied as a
 *  class on <html> so portaled layers (menus, popovers) inherit it too.
 *  Graphite is the token defaults in flowtag.scss; the rest override them. */

export type ThemeId = 'graphite' | 'carbon' | 'umber' | 'porcelain';

export interface ThemeMeta {
    id: ThemeId;
    name: string;
    hint: string;
    /** Picker swatch ramp: bg, panel, raised, text */
    swatch: readonly [string, string, string, string];
}

export const THEMES: readonly ThemeMeta[] = [
    { id: 'graphite', name: 'Graphite', hint: 'Pure-neutral dark', swatch: ['#111112', '#151517', '#1c1c1f', '#ededef'] },
    { id: 'carbon', name: 'Carbon', hint: 'True black, dark rooms', swatch: ['#000000', '#0b0b0c', '#161617', '#f2f2f4'] },
    { id: 'umber', name: 'Umber', hint: 'Warm dark, late nights', swatch: ['#141110', '#191512', '#211c16', '#f0e9df'] },
    { id: 'porcelain', name: 'Porcelain', hint: 'Light, for daylight', swatch: ['#f6f6f7', '#ffffff', '#eaeaed', '#1c1c21'] },
];

export const DEFAULT_THEME: ThemeId = 'graphite';

export function isThemeId(value: unknown): value is ThemeId {
    return THEMES.some((t) => t.id === value);
}

/** The theme currently applied to this window */
export function currentTheme(): ThemeId {
    const root = document.documentElement;
    return THEMES.find((t) => root.classList.contains(`th-${t.id}`))?.id ?? DEFAULT_THEME;
}

let crossfadeTimer: number | undefined;

/** Apply a theme to this window. `animated` runs the one authored moment:
 *  a 240ms surface crossfade (skipped under prefers-reduced-motion). */
export function applyTheme(theme: ThemeId, animated = false) {
    const root = document.documentElement;
    if (currentTheme() === theme) return;
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (animated && !reduced) {
        root.classList.add('theme-switching');
        window.clearTimeout(crossfadeTimer);
        crossfadeTimer = window.setTimeout(() => root.classList.remove('theme-switching'), 300);
    }
    for (const t of THEMES) {
        root.classList.toggle(`th-${t.id}`, t.id === theme);
    }
}

/** Tag / group swatches; mirrors TAG_COLOR_PALETTE in crates/onetagger-db/src/tags.rs */
export const TAG_COLORS: readonly string[] = [
    '#e5484d', // red
    '#f76b15', // orange
    '#ffb224', // amber
    '#92c94e', // lime
    '#30a46c', // green
    '#0091ff', // blue
    '#3e63dd', // indigo
    '#6e56cf', // violet
    '#8e4ec6', // purple
    '#d6409f', // magenta
    '#e93d82', // pink
    '#ad7f58', // bronze
];

const HEX6 = /^[0-9a-f]{6}$/;
const HEX3 = /^[0-9a-f]{3}$/;

/** '#rrggbb' (lowercase) from user text — '#rgb', 'rrggbb', '#RRGGBB' — or undefined */
export function parseHexColor(input: string): string | undefined {
    const raw = input.trim().toLowerCase().replace(/^#/, '');
    if (HEX6.test(raw)) return `#${raw}`;
    if (HEX3.test(raw)) return `#${raw.split('').map((c) => c + c).join('')}`;
    return undefined;
}

---
name: FlowTag
description: Linear-crisp pure-neutral dark system for the FlowTag library view and its player footer
colors:
  ink-bg: "#111112"
  ink-panel: "#151517"
  ink-raised: "#1c1c1f"
  ink-overlay: "#212124"
  hairline: "rgba(255, 255, 255, 0.07)"
  hairline-strong: "rgba(255, 255, 255, 0.14)"
  text-primary: "#ededef"
  text-secondary: "#a6a6ad"
  text-tertiary: "#717179"
  wash-hover: "rgba(255, 255, 255, 0.04)"
  wash-active: "rgba(255, 255, 255, 0.07)"
  focus-ring: "rgba(255, 255, 255, 0.4)"
  warn: "#d29922"
  danger: "#e5534b"
typography:
  headline:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 600
  title:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 600
  body:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', system-ui, sans-serif"
    fontSize: "12.5px"
    fontWeight: 500
    letterSpacing: "0.005em"
  label:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', system-ui, sans-serif"
    fontSize: "10.5px"
    fontWeight: 600
    letterSpacing: "0.08em"
  mono:
    fontFamily: "'SF Mono', ui-monospace, 'Cascadia Mono', Menlo, monospace"
    fontSize: "11.5px"
rounded:
  xs: "4px"
  sm: "5px"
  md: "6px"
  lg: "8px"
  pill: "11px"
spacing:
  xs: "4px"
  sm: "6px"
  md: "8px"
  lg: "10px"
  xl: "12px"
  2xl: "16px"
components:
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.md}"
    padding: "0 10px"
    height: "28px"
  button-ghost-hover:
    backgroundColor: "{colors.wash-hover}"
    textColor: "{colors.text-primary}"
  button-primary:
    backgroundColor: "{colors.ink-raised}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.md}"
    padding: "0 10px"
    height: "28px"
  button-primary-hover:
    backgroundColor: "{colors.ink-overlay}"
  field:
    backgroundColor: "{colors.ink-panel}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.md}"
    padding: "0 10px"
    height: "28px"
  chip:
    backgroundColor: "{colors.ink-panel}"
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.pill}"
    padding: "0 8px"
    height: "22px"
  kbd:
    backgroundColor: "{colors.ink-raised}"
    textColor: "{colors.text-tertiary}"
    rounded: "{rounded.xs}"
    padding: "3px 5px"
  popover:
    backgroundColor: "{colors.ink-overlay}"
    rounded: "{rounded.lg}"
    padding: "4px"
  track-row:
    backgroundColor: "transparent"
    textColor: "{colors.text-primary}"
    padding: "0 16px 0 14px"
    height: "40px"
  track-row-anchor:
    backgroundColor: "{colors.ink-raised}"
    rounded: "{rounded.md}"
---

# Design System: FlowTag

## Overview

**Creative North Star: "Linear-Crisp"**

FlowTag's visual world is a pure-neutral dark register: ink surfaces with no blue cast, hairline white-alpha borders, a tight small type ramp, and almost no chrome color. The interface is an instrument panel for a single power user working through thousands of tracks — density, legibility, and keyboard speed outrank expressiveness. The chrome stays monochrome so that the only color on screen is the user's own data: tag dots and Camelot key hues carry all the hue; the frame around them is grayscale.

Depth is conveyed by stepping surface lightness (`ink-bg` → `ink-panel` → `ink-raised` → `ink-overlay`) and hairline rings, not by color or heavy shadow. Real drop shadows exist only on floating layers (popovers, autocomplete). Interaction feedback is a white-alpha wash and a brightened border, never a colored glow. The build neutralizes Quasar's brand teal inside the theme (`--q-primary: #ededef`) and opts out of the app-wide Dosis font into the system UI stack.

**Scope.** This system governs the FlowTag view (`client/src/views/QuickTag.vue` and `client/src/components/flowtag/`) and the player footer on that route — everything under `.flowtag-theme` in `client/src/style/flowtag.scss`. Other views (AutoTagger, Audio Features, Tag Editor, Renamer) remain on the legacy upstream look and are out of scope.

**Key Characteristics:**
- Pure-neutral dark surfaces, no blue or teal cast anywhere in the chrome
- Color appears only through user data (tag colors, musical key colors)
- Hairline white-alpha borders; surface lightness steps carry structure
- Small, tight system-font type ramp (10.5–15px); tabular numerals for all data
- Fast, quiet transitions (80ms ease-out); keyboard hints as first-class UI

## Colors

An entirely neutral chrome palette — a four-step ink ramp, a three-step text ramp, white-alpha washes — plus two muted semantic colors used sparingly.

### Neutral
- **Ink Background** (`ink-bg`): The base canvas behind the track list, header bar, and tag panel.
- **Ink Panel** (`ink-panel`): One step up; fields, chips, the palette sidebar, segmented-control track, and the player footer.
- **Ink Raised** (`ink-raised`): Two steps up; primary buttons, keycaps, the anchor row, active segment, multi-select badge.
- **Ink Overlay** (`ink-overlay`): The highest surface; floating popovers and autocomplete panels only.
- **Hairline** (`hairline`): Default border for fields, chips, keycaps, section dividers (header-bar bottom, tag-panel top, palette left, footer top).
- **Hairline Strong** (`hairline-strong`): Border for raised/primary elements, popover edges, the anchor row's inset ring, and all hover border-brightening.
- **Text Primary** (`text-primary`): Track titles, field input text, active states. Also the value `--q-primary` is pinned to inside the theme.
- **Text Secondary** (`text-secondary`): Default button/chip/suggestion text, counts, BPM, artists in the row list are one step below.
- **Text Tertiary** (`text-tertiary`): Artists, placeholders, labels, keycaps, counts' dim halves, hints, the progress fill, and the ungrouped-section dot.
- **Wash Hover / Wash Active** (`wash-hover`, `wash-active`): White-alpha fills for hover and pressed/selected states (range-selected rows use `wash-active`).
- **Focus Ring** (`focus-ring`): `:focus-visible` outline and focused field borders — a brightened neutral, never a hue.

### Semantic
- **Warn** (`warn`): A muted amber used for exactly one thing: the 5px pending-write dot on rows whose tags haven't been flushed to the file yet.
- **Danger** (`danger`): A muted red reserved for destructive affordances. Defined in the token layer; use sparingly.

### Data Color (not chrome)
Tag colors are user-assigned and render only as small dots (6–8px) plus neutral-text labels — never as chip fills. Musical key text takes its hue from the Camelot wheel ramp in `client/src/scripts/musickey.ts` (`KEY_COLORS`, e.g. 8A/8B `#fe3fea`, 1A/1B `#00d58f`). When a chip background must carry a user color, `chipTextColor()` in `TagChips.vue` picks black or white by relative luminance.

### Named Rules
**The Data-Only Color Rule.** The chrome is monochrome neutral. The only hue on screen comes from the user's data: tag colors as dots, Camelot key colors as text. Never introduce an accent color into buttons, borders, focus states, or backgrounds.

**The Two-Semantic Rule.** Exactly two semantic colors exist: `warn` for pending writes and `danger` for destructive actions. No success green, no info blue.

## Typography

**UI Font:** system stack (`-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', system-ui, sans-serif`) — SF on macOS
**Data/Mono Font:** `SF Mono, ui-monospace, Cascadia Mono, Menlo, monospace`

**Character:** Crisp, small, and dense — a native-feeling instrument panel. The theme forcibly opts out (`!important`, excepting `.wavefont`) of the app's global Dosis face; the rest of the app still forces Dosis, and that boundary is intentional.

### Hierarchy
- **Headline** (600, 15px): The largest text in the system; empty-state titles only.
- **Title** (600, 13px): Selected-track title in the tag panel. Track-row titles are a half step quieter (500, 13px); the tag input also sits at 13px.
- **Body** (500, 12.5px, 0.005em): Buttons, fields, chips (11–11.5px dense), suggestions, artists, hints. 400 for passive copy.
- **Label** (600, 10.5px, 0.08em, UPPERCASE): Section labels (`.ft-label`, palette section headers). Tertiary text color.
- **Mono** (`.ft-mono`, 11.5px; footer 11px; keycaps 10px): Musical keys, player timecodes, keyboard hints.

### Named Rules
**The Tabular Numerals Rule.** Every data numeral — counts, BPM, year, tag counts, timecodes — is set tabular (`.ft-num` or `.ft-mono`, `font-variant-numeric: tabular-nums`) so columns never shimmy as values change.

**The System Face Rule.** Inside `.flowtag-theme`, everything except `.wavefont` uses the system UI stack. Dosis never appears inside the theme; the system stack never leaks outside it.

## Layout

A fixed vertical shell: 46px header bar (12px horizontal padding), a 2px session-progress line, a flexible track-list region with an optional 240px right palette column, and a tag panel of min-height 52px (16px horizontal padding, 12px gap) above the player footer. Track rows are strictly single-line at 40px with 10px internal gap; titles cap at 42% width, chips at 280px, metadata columns are fixed-width right-aligned (34px meta, 30px key). Density is constant — there is no responsive breakpoint behavior; this is a desktop webview at one density. Spacing rhythm runs in small, even steps (4/6/8/10/12/16px); control heights are 28px (buttons, fields, suggestions), 26px (palette rows), 24px (filter chips), 22px (chips; 20px dense).

## Elevation & Depth

Depth is primarily tonal: the four-step ink ramp plus hairline borders does nearly all the work. Flat surfaces sit on `ink-bg`; contained elements step up to `ink-panel`; emphasized or active elements to `ink-raised` with a `hairline-strong` ring (the anchor row uses `box-shadow: inset 0 0 0 1px` rather than a border, and no colored rail). Real drop shadows exist only on floating layers.

### Shadow Vocabulary
- **Popover** (`box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45), 0 2px 6px rgba(0, 0, 0, 0.3)`): The only drop shadow in the system; applied via `.ft-pop` and the tag-input suggestions panel, always paired with `ink-overlay` background and `hairline-strong` border.
- **Filter halo** (`box-shadow: 0 0 0 2px var(--ft-bg), 0 0 0 3px var(--ft-text-3)`): A neutral double-ring around a palette tag dot while it filters the list.

### Named Rules
**The Float-Only Shadow Rule.** Shadows mark things that float above the page (menus, autocomplete). In-flow elements express elevation with surface lightness and hairline rings, never with drop shadows.

## Shapes

Rectilinear with quiet rounding: 6px is the workhorse radius (buttons, fields, anchor row, tag input), 4–5px for small/inner elements (keycaps, segment buttons, palette rows, suggestions, scrollbar thumbs), 8px for floating panels, and a full pill (11px on 22px height) for tag chips only. Borders are 1px hairlines everywhere; the single playful detail is the keycap's 2px bottom border suggesting a physical key. Dots are perfect circles (5–8px). No other shape devices — no angled cuts, no heavy outlines.

## Components

### Buttons (`.ft-btn`)
- **Character:** Ghost-quiet until touched.
- **Shape:** Gently rounded (6px), 28px tall, 0 10px padding, 12.5px/500 text.
- **Ghost (default):** Transparent, `text-secondary`; hover fills `wash-hover` and brightens to `text-primary`; active fills `wash-active`.
- **Primary (`.ft-btn-primary`):** `ink-raised` fill, `hairline-strong` border, `text-primary`; hover steps to `ink-overlay`. Still neutral — "primary" means raised, not colored.
- **Motion:** `background`/`color` transitions at 80ms ease-out.

### Fields (`.ft-field`, tag input)
- **Style:** `ink-panel` fill, 1px `hairline` border, 6px radius, 28px tall; placeholder in `text-tertiary`.
- **Focus:** Border brightens to `focus-ring` — no colored glow, no box-shadow. Hover brightens to `hairline-strong`.
- **Adornments:** Leading icon in `text-tertiary`; a trailing `.ft-kbd` keycap shows the shortcut when empty.

### Chips (`.flowtag-chip`)
- **Anatomy:** dot + label (+ optional × remove button). 22px pill (20px dense), `ink-panel` fill, `hairline` border.
- **Color:** The 6–7px dot carries the tag's color; the label stays neutral (`text-secondary`, 11.5px/500). Hover brightens border and label.

### Keycaps (`.ft-kbd`)
- 10px mono, `text-tertiary` on `ink-raised`, 4px radius, hairline border with a 2px bottom edge. Used inline in hints, fields, and the palette.

### Popovers (`.ft-pop`, suggestions)
- `ink-overlay` surface, `hairline-strong` border, 8px radius, the system's one drop shadow, 4px inner padding. Rows are 28px, 5px radius, highlighted with `wash-active` + `text-primary`.

### Track Rows (`.flowtag-row`)
- **Rest:** 40px single line, 4%-alpha bottom hairline, default cursor; hover is `wash-hover`.
- **Range-selected:** quiet `wash-active` fill, title lifts to white.
- **Anchor (active):** `ink-raised` surface with a 1px inset `hairline-strong` ring and 6px radius — raised, not colored; no accent rail.
- **Data cells:** year/BPM tabular (`.ft-num`), key in `.ft-mono` 600 colored by `keyColor()`; pending writes show a 5px `warn` dot.

### Segmented Control (`.seg`)
- `ink-panel` track with `hairline` border (6px radius, 2px padding); 12px/500 buttons at 4px radius; the active segment sits on `ink-raised` in `text-primary`.

### Palette Rows (`.pal-row`)
- 26px rows: 8px dot, 12.5px/500 name, keycap hint, tabular count. Hover `wash-hover`; active `wash-active` + 600 weight; filtering state uses `ink-raised` + inset ring + the dot's neutral double-ring halo; disabled at 0.45 opacity.

### Themed Browser Surfaces
- Scrollbars: 10px, transparent track, white-alpha 12% content-box thumb (22% on hover), 5px radius. Selection: white-alpha 20%. Caret: `text-primary`. `:focus-visible`: 2px `focus-ring` outline, 1px offset, 4px radius.

### Player Footer (`.q-footer.flowtag-theme`)
- Joins the theme on the FlowTag route: `ink-panel` background, `hairline` top border; timecodes in 11px mono, tabular, `text-tertiary`.

## Do's and Don'ts

### Do:
- **Do** keep all chrome monochrome; the only color sources are tag dots and Camelot key text (The Data-Only Color Rule).
- **Do** set every data numeral tabular via `.ft-num` or `.ft-mono`.
- **Do** express emphasis by raising the surface one ink step and adding a `hairline-strong` ring (anchor row, primary button, filtering palette row).
- **Do** use 80ms ease-out for interaction transitions and reserve longer motion (300ms) for passive indicators like the progress line.
- **Do** keep keycap hints (`.ft-kbd`) visible in hints and fields — keyboard-first is a product constraint.
- **Do** scope every new FlowTag style under `.flowtag-theme` and use the `--ft-*` tokens rather than literal values.

### Don't:
- **Don't** introduce accent-colored buttons, borders, glows, or focus rings; focus brightens a neutral border, nothing more.
- **Don't** let Quasar's teal or the Dosis font leak into the theme (`--q-primary` is pinned to `text-primary`; the font opt-out is deliberate).
- **Don't** add drop shadows to in-flow elements; shadows belong to floating panels only (The Float-Only Shadow Rule).
- **Don't** fill chips with the tag's color; the dot carries the hue, the label stays neutral.
- **Don't** add semantic colors beyond `warn` and `danger` (The Two-Semantic Rule).
- **Don't** restyle the legacy views (AutoTagger, Tag Editor, Renamer) with this system; its scope ends at `.flowtag-theme`.

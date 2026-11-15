---
name: FlowTag
description: Linear-crisp neutral dark system for the FlowTag library view and its player footer, with eight token-swap themes
colors:
  bg: "#111112"
  panel: "#151517"
  raised: "#1c1c1f"
  overlay: "#212124"
  line: "rgba(255, 255, 255, 0.07)"
  line-strong: "rgba(255, 255, 255, 0.14)"
  line-faint: "rgba(255, 255, 255, 0.04)"
  stripe: "rgba(255, 255, 255, 0.02)"
  selection: "rgba(255, 255, 255, 0.2)"
  scroll-thumb: "rgba(255, 255, 255, 0.12)"
  text: "#ededef"
  text-2: "#a6a6ad"
  text-3: "#888890"
  text-bright: "#ffffff"
  hover: "rgba(255, 255, 255, 0.04)"
  active: "rgba(255, 255, 255, 0.07)"
  focus: "rgba(255, 255, 255, 0.4)"
  warn: "#d29922"
  danger: "#e5534b"
  playing: "#30a46c"
typography:
  headline:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 600
  title:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', system-ui, sans-serif"
    fontSize: "13.5px"
    fontWeight: 600
    letterSpacing: "0.01em"
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
    textColor: "{colors.text-2}"
    rounded: "{rounded.md}"
    padding: "0 10px"
    height: "28px"
  button-ghost-hover:
    backgroundColor: "{colors.hover}"
    textColor: "{colors.text}"
  button-primary:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    padding: "0 10px"
    height: "28px"
  button-primary-hover:
    backgroundColor: "{colors.overlay}"
  field:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    padding: "0 10px"
    height: "28px"
  chip:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.text-2}"
    rounded: "{rounded.pill}"
    padding: "0 8px"
    height: "22px"
  kbd:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text-3}"
    rounded: "{rounded.xs}"
    padding: "3px 5px"
  popover:
    backgroundColor: "{colors.overlay}"
    rounded: "{rounded.lg}"
    padding: "4px"
  track-row:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    padding: "0 16px 0 14px"
    height: "40px"
  track-row-anchor:
    backgroundColor: "{colors.raised}"
    rounded: "{rounded.md}"
---

# Design System: FlowTag

## Overview

**Creative North Star: "Linear-Crisp"**

FlowTag's visual world is a neutral dark register: ink surfaces, hairline alpha borders, a tight small type ramp, and almost no chrome color. The interface is an instrument panel for a single power user working through thousands of tracks — density, legibility, and keyboard speed outrank expressiveness. The chrome stays quiet so that the color that matters on screen is the user's own data: tag dots and Camelot key hues carry the meaning. Eight selectable themes re-voice the same system by swapping the `--ft-*` token values — including four "colorful shelf" themes that tint the interaction layer (focus, washes, selection, waveform) with a single hue while the row field stays quiet.

Depth is conveyed by stepping surface lightness (`bg` → `panel` → `raised` → `overlay`) and hairline rings, not by color or heavy shadow. Real drop shadows exist only on floating layers (popovers, autocomplete, the narrow-window overlay panels). Interaction feedback is an alpha wash and a brightened border. The stack is Vue 3 + Tailwind v4 + shadcn-vue (reka-ui) + Lucide; two component vocabularies coexist deliberately — hand-rolled `.ft-btn` / `.ft-field` / chip primitives for the dense instrument surfaces, and shadcn primitives (owned code, retuned to the `--ft-*` tokens) for dialogs, menus, and inputs.

**Scope.** This system governs the FlowTag view (`client/src/views/QuickTag.vue` and `client/src/components/flowtag/`) and the player footer on that route — everything under `.flowtag-theme` plus `.ft-footer` in `client/src/style/flowtag.scss`. Other views (AutoTagger, Audio Features, Tag Editor, Renamer) remain on the legacy upstream look and are out of scope.

**Key Characteristics:**
- Quiet neutral chrome; themes re-tint the whole system by token swap, never per-component
- Data color (tag dots, musical key hues) is the user's; chrome hue comes only from the active theme's token set
- Hairline alpha borders; surface lightness steps carry structure; a ~2%-alpha zebra stripe textures the list
- Small, tight system-font type ramp (10.5–15px); tabular numerals for all data
- Fast, quiet transitions (80ms ease-out); keycap hints as first-class UI, with a press echo

## Colors

A neutral chrome palette — a four-step surface ramp, a three-step text ramp, alpha washes and hairlines — plus three muted semantic colors used sparingly. The Graphite values in the frontmatter are the normative base set; every theme overrides the same tokens.

### Neutral
- **Ink Background** (`bg`): The base canvas behind the track list, header bar, and tag panel.
- **Ink Panel** (`panel`): One step up; fields, chips, the palette and playlist sidebars, segmented-control track, progress-line track, and the player footer.
- **Ink Raised** (`raised`): Two steps up; primary buttons, keycaps, the anchor row, active segment, active playlist row, multi-select badge.
- **Ink Overlay** (`overlay`): The highest surface; floating popovers, menus, and autocomplete panels only.
- **Hairline** (`line`): Default border for fields, chips, keycaps, and section dividers (header-bar bottom, tag-panel top, palette left, footer top, `.hdr-rule`).
- **Hairline Strong** (`line-strong`): Border for raised/primary elements, popover edges, inset rings, overlay panel edges, and all hover border-brightening.
- **Hairline Faint** (`line-faint`): The per-row bottom hairline in the track list — separation at the threshold of visibility.
- **Stripe** (`stripe`): The zebra fill on odd track rows (~2% alpha) — deliberately subliminal texture; hairlines carry the actual row separation, and hover/selected/anchor all out-rank it.
- **Text** / **Text 2** / **Text 3** (`text`, `text-2`, `text-3`): The three-step text ramp — titles and active states; default button/chip/suggestion text and BPM; artists, placeholders, labels, keycaps, counts, hints, the progress fill. `text-bright` (pure white) lifts selected-row titles. Every theme's `text-3` clears 4.5:1 on its `raised` surface.
- **Hover / Active** (`hover`, `active`): Alpha washes for hover and pressed/selected states (range-selected rows and the active palette row use `active`).
- **Focus** (`focus`): The one focus indicator — a 2px outline on `:focus-visible` (text fields show focus with their border instead). In the colorful-shelf themes this is a theme-hued ring.
- **Selection / Scroll Thumb** (`selection`, `scroll-thumb`): Browser surfaces carry the theme too — text selection, scrollbar thumbs, and the caret are token-driven.

### Semantic
- **Warn** (`warn`): Muted amber for unfinished business: the 5px pending-write dot, the "N pending" flush count, the skip pill, and the staged-with-no-tags mark.
- **Danger** (`danger`): Muted red reserved for destructive affordances — delete/remove rows in menus and suggestions, write-error signage, the destructive button variant.
- **Playing Green** (`playing`): The system's progress color: the playing-track speaker icon, the done ✓ mark, the header done-count chip, the Journal chart bars, and the sidebar stage-progress fill. Playback and completed work share one green.

### Data Color (not chrome)
Tag colors are user-assigned and render only as small dots (6–8px) plus neutral-text labels — never as chip fills. Musical key text takes its hue from the Camelot wheel ramp in `client/src/scripts/musickey.ts` (`KEY_COLORS`, e.g. 8A/8B `#fe3fea`, 1A/1B `#00d58f`), set via a `--kc` custom property on the cell. When a chip background must carry a user color, `chipTextColor()` in `TagChips.vue` picks black or white by relative luminance. Themes never touch data color; Porcelain only compensates its contrast for the light ground (`color-mix(in oklab, var(--kc) 72%, #16161a)` on key text, a hairline ring on pale tag dots) while preserving hue.

### Themes
Eight themes live in `flowtag.scss` as value sets for the same `--ft-*` tokens, registered in `client/src/scripts/themes.ts` and applied as a class on `<html>` (`th-carbon`, `th-porcelain`, …) so portaled layers follow. Graphite is the default token set; the others override it:

- **Graphite** — pure-neutral dark (the normative frontmatter values).
- **Carbon** — true black for dark rooms; hairlines and text step up so edges survive the void.
- **Umber** — warm dark for late nights; the whole alpha family rides a candle tint and focus is the one amber note.
- **Porcelain** — the one light theme; the same hairline logic on an ink-on-paper ramp, with its own composed shadows (never the dark themes') and the data-color compensation above.
- **The colorful shelf** — **Blue Hour** (indigo night, electric-blue interaction layer), **UV** (blacklight purple-black, magenta), **Signal** (near-neutral charcoal, hot amber), **Phosphor** (green-black CRT, where the accent and `playing` unify into one phosphor green, deliberately). In these, the hue lives in the interaction layer — focus ring, selection, washes, hairlines, waveform — while surfaces and row text stay quiet.

Switching themes runs the one authored theme moment: a 240ms surface crossfade (`html.theme-switching`), skipped under `prefers-reduced-motion`.

### Named Rules
**The Data-Only Color Rule.** Chrome takes hue only from the active theme's token set; data hue belongs to the user. Tag colors appear as dots, Camelot colors as key text, and no component ever hard-codes a chrome hue outside the `--ft-*` tokens.

**The Three-Semantic Rule.** Exactly three semantic colors exist: `warn` for unfinished business (pending writes, skip, staged-with-no-tags), `danger` for destructive actions, and `playing` green for playback and completed work (done ✓, done count, journal bars, stage progress). No fourth semantic, no info blue.

**The Token-Swap Rule.** A theme is a value set for the existing `--ft-*` tokens applied on `<html>` — never a parallel stylesheet, never per-component overrides. New chrome styles must read tokens so every theme inherits them for free.

## Typography

**UI Font:** system stack (`-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', system-ui, sans-serif`) — SF on macOS
**Data/Mono Font:** `SF Mono, ui-monospace, Cascadia Mono, Menlo, monospace`

**Character:** Crisp, small, and dense — a native-feeling instrument panel. The theme root sets the system stack directly; nothing inside `.flowtag-theme` uses the legacy app's display face.

### Hierarchy
- **Headline** (600, 15px): The largest text in the system; empty-state titles only.
- **Title** (`.ft-title`, 600, 13.5px, 0.01em): Dialog and window titles. The tag-panel track title and track-row titles sit a half step down (600 and 500 at 13px); the tag input is also 13px.
- **Body** (500, 12.5px, 0.005em): Buttons, fields, palette rows, suggestions, artists, hints. Dense metadata runs 10.5–12px in `text-3`; 400 for passive copy.
- **Label** (`.ft-label`, 600, 10.5px, 0.08em, UPPERCASE): Section labels (palette and sidebar section headers). Tertiary text color.
- **Mono** (`.ft-mono`, 11.5px; footer timecodes 11px; keycaps 10px): Musical keys, player timecodes, keyboard hints.

### Named Rules
**The Tabular Numerals Rule.** Every data numeral — counts, BPM, year, added dates, tag counts, timecodes, progress text — is set tabular (`.ft-num`, `.ft-mono`, or Tailwind `tabular-nums`) so columns never shimmy as values change.

**The One-Line Rule.** Header and row labels never wrap: `white-space: nowrap` is inherited across the header bar, squeezed labels shrink or fold into the overflow menu, and truncation is always an ellipsis.

## Layout

A fixed vertical shell: 46px header bar (12px horizontal padding), a 2px session-progress line, a flexible middle row (optional playlist sidebar · virtualized track list · optional 240px right palette column), and a tag panel of min-height 52px (16px horizontal padding, 12px gap) above the player footer. Track rows are strictly single-line at 40px with 10px internal gap; titles cap at 42% width, chips at 280px, metadata columns are fixed-width right-aligned (34px meta, 30px key, 74px added, 200px path).

Density is constant, but the frame has a three-step responsive ladder: **full ≥1200px**, **medium 900–1199px** (path/added columns fold, header buttons shorten or move into the ⋯ overflow menu, the search field absorbs the squeeze), and **narrow <900px** (year folds, chips cap at one + "+n", the sidebars become overlay panels over a 40%-black scrim with directional shadows). Folded controls stay one click away and every shortcut keeps working. Spacing rhythm runs in small, even steps (4/6/8/10/12/16px); control heights are 28px (buttons, fields, suggestions, menu items), 26px (palette rows, sort select, direction button), 24px (filter chips), 22px (chips; 20px dense).

## Elevation & Depth

Depth is primarily tonal: the four-step surface ramp plus hairline borders does nearly all the work. Flat surfaces sit on `bg`; contained elements step up to `panel`; emphasized or active elements to `raised` with a `line-strong` ring (the anchor row and filtering palette row use `box-shadow: inset 0 0 0 1px` rather than a border, and no colored rail). Real drop shadows exist only on floating layers. Porcelain composes its own lighter shadows rather than reusing the dark themes'.

### Shadow Vocabulary
- **Popover** (`--ft-shadow-pop`: `0 8px 24px rgba(0, 0, 0, 0.45), 0 2px 6px rgba(0, 0, 0, 0.3)`; Porcelain: `0 8px 24px rgba(20, 20, 30, 0.14), 0 2px 6px rgba(20, 20, 30, 0.1)`): Floating panels — `.ft-pop`, context menus, autocomplete — always paired with `overlay` background and `line-strong` border.
- **Overlay panel** (`box-shadow: ±12px 0 32px rgba(0, 0, 0, 0.45)`): The narrow-window sidebar/palette overlays, cast toward the list they cover, with a `line-strong` edge.
- **Filter halo** (`box-shadow: 0 0 0 2px var(--ft-bg), 0 0 0 3px var(--ft-text-3)`): A neutral double-ring around a palette tag dot while it filters the list.

### Named Rules
**The Float-Only Shadow Rule.** Shadows mark things that float above the page (menus, autocomplete, overlay panels). In-flow elements express elevation with surface lightness and hairline rings, never with drop shadows.

## Shapes

Rectilinear with quiet rounding: 6px is the workhorse radius (buttons, fields, anchor row, tag input, segmented track, menu items), 4–5px for small/inner elements (keycaps, segment buttons, palette rows, suggestions, multi-badge, scrollbar thumbs), 8px for floating panels, and a full pill (11px on 22px height) for tag chips only. Borders are 1px hairlines everywhere; the single playful detail is the keycap's 2px bottom border suggesting a physical key — which collapses to 1px with a 1px dip when its bound key is pressed (`.kbd-echo`). Vertical rules are 1px × 14–16px hairlines (`.hdr-rule` in the header, `.hint-rule` in the hint line). Dots are perfect circles (5–8px). No other shape devices — no angled cuts, no heavy outlines.

## Components

Two vocabularies, one skin: hand-rolled `ft-` primitives for the dense always-visible surfaces, and shadcn-vue primitives (owned code in `client/src/components/ui/`, retuned to the `--ft-*` tokens via the Tailwind bridge in `style/tailwind.css`) for dialogs, menus, popovers, and form controls. Both read the same tokens, so themes restyle everything at once.

### Buttons (`.ft-btn`, shadcn `Button`)
- **Character:** Ghost-quiet until touched. Ghost is the default in both vocabularies.
- **Shape:** Gently rounded (6px), 28px tall, 0 10px padding, 12.5px/500 text; icon-only variants tighten to 0 5–6px padding.
- **Ghost (default):** Transparent, `text-2`; hover fills `hover` wash and brightens to `text`; active fills `active`.
- **Primary (`.ft-btn-primary`, shadcn `default`):** `raised` fill, `line-strong` border, `text`; hover steps to `overlay`. Still neutral — "primary" means raised, not colored.
- **Destructive (shadcn):** `danger` text on a ghost body; red never fills a button.
- **Motion:** `background`/`color` transitions at 80ms ease-out; focus comes from the global `:focus-visible` outline.

### Fields (`.ft-field`, `.ft-inline-input`, tag input)
- **Style:** `panel` fill, 1px `line` border, 6px radius, 28px tall; placeholder in `text-3`.
- **Focus:** Border brightens to `focus` — no glow, no box-shadow. Hover brightens to `line-strong`.
- **Adornments:** Leading icon in `text-3`; a trailing `.ft-kbd` keycap shows the shortcut when empty.
- **Inline edit (`.ft-inline-input`):** Reads as plain text until hovered (wash) or focused (border + `panel` fill) — used for tag/group renames.

### Chips (`.flowtag-chip`)
- **Anatomy:** dot + label (+ optional × remove button). 22px pill (20px dense), `panel` fill, `line` border, 150px max label with ellipsis.
- **Color:** The 6–7px dot carries the tag's color; the label stays neutral (`text-2`, 11.5px/500). Hover brightens border and label. Overflow renders as a "+n" count whose `title` lists the hidden tags.
- **Filter chips** in the header are the same chip at 24px with an × that clears the filter.

### Keycaps (`.ft-kbd`)
- 10px mono, `text-3` on `raised`, 4px radius, hairline border with a 2px bottom edge. Used inline in hints, fields, the palette, and the sidebar's stage-keys line. Pressing the bound key dips the on-screen cap for a beat (translateY(1px) + bottom edge 2px→1px, 80ms) via `scripts/keycapecho.ts`.

### Popovers & Menus (`.ft-pop`, `.ft-ctx`, suggestions, shadcn Dropdown/Popover)
- `overlay` surface, `line-strong` border, 8px radius, the popover shadow, 4px inner padding. Rows are 28px at 5–6px radius, highlighted with `hover`/`active` wash + `text`; destructive rows in `danger` text; disabled rows at 0.5 opacity.
- The tag-input autocomplete opens upward above the tag panel and follows the same recipe.

### Track Rows (`.flowtag-row`)
- **Rest:** 40px single line, `line-faint` bottom hairline, default cursor; odd rows carry the `stripe` zebra fill; hover is `hover` wash.
- **Range-selected:** quiet `active` fill; title lifts to `text-bright`.
- **Anchor (active):** `raised` surface with a 1px inset `line-strong` ring and 6px radius — raised, not colored; no accent rail.
- **State marks (right of the chips):** done is a bare ✓ in `playing` green; skip is a hairline pill in `warn`; a plain staged row shows the quiet staged mark — a 12px Lucide LayersIcon in `text-3`, stepping to `warn` only when staged-with-no-tags (the one state worth spotting). Pending file writes show the 5px `warn` dot; a missing file shows a `text-3` icon with a Hint.
- **Data cells:** year/BPM/added tabular (`.ft-num`), path leading-truncated, key in `.ft-mono` 600 colored via `--kc` from `keyColor()`.
- **Playing:** a 12px `playing`-green speaker icon tucks before the title.
- **Semantics:** the list is an ARIA listbox (`role="listbox"` / `role="option"`, `aria-activedescendant`); focus never moves off the list — a `sr-only` polite live region echoes the anchor row (title, artist, tags) as J/K moves it.

### Segmented Control (`.seg`) and the Ghost Sort Select
- `panel` track with `line` border (6px radius, 2px padding); 12px/500 buttons at 4px radius; the active segment sits on `raised` in `text`. The bordered seg is **grouped state** (scope: All / Not done / Untagged).
- The sort select beside it is deliberately a ghost control (transparent, borderless, `text-2`, hover wash) — ghost is the **action** vocabulary; the contrast between bordered seg and ghost select is intentional, not drift.

### Palette Rows (`.pal-row`)
- 26px rows under uppercase `.ft-label` section headers (each with a 7px group-color dot; the ungrouped section's dot is `text-3`): 8px dot, 12.5px/500 name, keycap hint, tabular count. Hover `hover`; active `active` + 600 weight; filtering state uses `raised` + inset ring + the dot's neutral double-ring halo; disabled at 0.45 opacity. Each row's full behavior lives in its native `title` tooltip.

### Playlist Sidebar
- `panel` surface with a `line` right border; 28px rows with a 14px `text-3` icon, hover wash, `raised` active state, and a `focus`-colored inset ring as the drag-drop target. The Stage row carries a `line` progress track with a `playing`-green fill and a `stage-keys` keycap line (`S` stage · `X` skip · `⌘↵` commit) in `text-3`.

### Header Signage (ScanStatus, ExportStatus, FlushIndicator, done chip)
- Long-running work shows on-surface in the header, not in toasts: a 120px shadcn Progress bar + tabular "Scanning…/Exporting… n/N" in `text-3`; flush state shows "N pending" in `warn` and a `danger` "N failed" popover with retry; the done-count chip renders in `playing` green and opens the Journal.

### Hints & Tooltips
- Dense repeated rows (palette rows, chips, truncated cells) use the native `title` attribute; singular controls use the `Hint` component (shadcn Tooltip in FlowTag skin). Keycap hints stay visible in the tag panel's hint line, separated by 1px `line-strong` rules.

### Themed Browser Surfaces
- Scrollbars: 10px, transparent track, `scroll-thumb` content-box thumb (hover steps up), 5px radius. Selection: `selection`. Caret: `text`. `:focus-visible`: 2px `focus` outline, 1px offset, following each element's own radius.

### Player Footer (`.ft-footer`)
- `panel` background, `line` top border; timecodes in 11px mono, tabular, `text-3`. The waveform reads the `--wave-played` / `--wave-hover` / `--wave-rest` tokens, so colorful-shelf themes tint the played range with the theme hue.

## Do's and Don'ts

### Do:
- **Do** keep all chrome on the `--ft-*` tokens; the only hard-coded hue on screen is the user's data — tag dots and Camelot key text (The Data-Only Color Rule, The Token-Swap Rule).
- **Do** set every data numeral tabular via `.ft-num`, `.ft-mono`, or `tabular-nums`.
- **Do** express emphasis by raising the surface one step and adding a `line-strong` ring (anchor row, primary button, filtering palette row).
- **Do** use 80ms ease-out for interaction transitions and reserve longer motion for passive indicators (300ms progress line) and the one authored moment (240ms theme crossfade) — both reduced-motion-safe.
- **Do** keep keycap hints (`.ft-kbd`) visible in hints, fields, and the sidebar — keyboard-first is a product constraint — and let bound keys echo on their caps.
- **Do** use `warn` for unfinished business, `danger` for destruction, and `playing` green for playback and completed work — nothing else (The Three-Semantic Rule).
- **Do** use native `title` tooltips on dense repeated rows and the `Hint` component on singular controls.
- **Do** show long-running work on-surface in the header (progress bar + tabular count), not in toasts.

### Don't:
- **Don't** hard-code a chrome color in a component; a theme must be able to re-voice it by token swap alone. Theme-hued focus rings, washes, and hairlines are legitimate — but only via the tokens.
- **Don't** add drop shadows to in-flow elements; shadows belong to floating layers only (The Float-Only Shadow Rule).
- **Don't** fill chips with the tag's color; the dot carries the hue, the label stays neutral.
- **Don't** add a fourth semantic color, or spend `playing` green on anything that isn't playback or completed work.
- **Don't** retint user data colors per theme; only Porcelain's contrast compensation (hue-preserving color-mix, dot rings) touches them.
- **Don't** let header or row labels wrap; shrink, truncate with ellipsis, or fold into the ⋯ overflow menu.
- **Don't** restyle the legacy views (AutoTagger, Tag Editor, Renamer) with this system; its scope ends at `.flowtag-theme` and `.ft-footer`.

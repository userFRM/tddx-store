---
name: TdDx Store
description: |
  TdDx Store is a desktop dataset downloader for ThetaData market data.
  Users browse, preview, and download structured tick / quote / OHLC /
  greeks datasets across stocks, options, indices, and rates.
identity:
  archetype: "ThetaData product surface, at working density"
  source_of_truth: "ThetaData UI foundations v1 (board F01, Precision Catalog)"
  references:
    - ThetaData website atlas (brand palette, type, spacing, actions)
    - Bloomberg Terminal (information density, monospaced precision)
    - Hugging Face Datasets (browse to preview to download flow)
colors:
  brand-blue: "#1074FF"   # Identity. Logo, active indicators, focus rings.
  accent: "#0063CC"       # Action blue. Links and filled buttons.
  accent-hi: "#0052A8"    # Hover. Darkens, per the brand sheet.
  accent-lo: "#004387"    # Pressed.
  accent-tint: "rgba(16, 116, 255, 0.10)"
  bg: "#FAFAFA"           # Canvas.
  surface-1: "#FFFFFF"    # Cards, sidebars.
  surface-2: "#F4F5F7"    # Hover, raised cards.
  surface-3: "#EDEFF3"    # Modals, popovers.
  border: "#E0E0E0"       # Divider.
  border-strong: "#C9CDD4"
  fg: "#323348"           # Text.
  fg-muted: "#525252"     # Brand gray.
  fg-subtle: "#8A8D9B"
  good: "#1F7A4D"
  warn: "#8A5B00"
  bad: "#C0392B"
  data-bull: "#0F8A55"
  data-bear: "#C0392B"
typography:
  display-xl:
    fontFamily: "Inter, system-ui"
    fontSize: "2rem"
    fontWeight: 600
    letterSpacing: "-0.02em"
    lineHeight: 1.1
  display-lg:
    fontFamily: "Inter, system-ui"
    fontSize: "1.5rem"
    fontWeight: 600
    letterSpacing: "-0.015em"
    lineHeight: 1.15
  heading:
    fontFamily: "Inter, system-ui"
    fontSize: "1rem"
    fontWeight: 600
    letterSpacing: "-0.005em"
    lineHeight: 1.4
  body:
    fontFamily: "Inter, system-ui"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.5
  body-sm:
    fontFamily: "Inter, system-ui"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.45
  caption:
    fontFamily: "Inter, system-ui"
    fontSize: "0.6875rem"
    fontWeight: 500
    letterSpacing: "0.04em"
    textTransform: "uppercase"
    lineHeight: 1.4
    color: "fg-muted"
  mono:
    fontFamily: "IBM Plex Mono, SF Mono, Consolas, ui-monospace, monospace"
    fontSize: "0.8125rem"
    fontVariantNumeric: "tabular-nums"
    fontWeight: 400
    lineHeight: 1.4
rounded:
  none: "0px"
  sm: "6px"     # Inputs, per the brand sheet.
  md: "8px"     # Cards, per the brand sheet.
  lg: "12px"
  xl: "16px"
  pill: "999px"
spacing:
  px: "1px"
  0: "0"
  1: "4px"
  2: "8px"
  3: "12px"
  4: "16px"
  5: "20px"
  6: "24px"
  8: "32px"
  10: "40px"
  12: "48px"
  16: "64px"
  24: "96px"
shadows:
  flat: "0 0 0 1px rgba(50,51,72,0.06)"
  raised: "0 1px 2px rgba(50,51,72,0.08), 0 0 0 1px rgba(50,51,72,0.05)"
  modal: "0 24px 64px rgba(50,51,72,0.18), 0 0 0 1px rgba(50,51,72,0.08)"
  glow-accent: "0 0 0 3px rgba(16,116,255,0.25)"
motion:
  duration:
    instant: "60ms"
    fast: "120ms"
    base: "150ms"
    slow: "260ms"
  easing:
    standard: "cubic-bezier(0.2, 0.0, 0, 1.0)"
    decel:    "cubic-bezier(0.0, 0.0, 0.2, 1.0)"
    accel:    "cubic-bezier(0.4, 0.0, 1.0, 1.0)"
breakpoints:
  sm: "640px"
  md: "960px"
  lg: "1280px"
  xl: "1600px"
icon:
  library: "lucide-svelte"
  size:
    sm: "14px"
    md: "16px"
    lg: "20px"
  stroke: 1.5
---

## Identity

TdDx Store downloads ThetaData market data, so it looks like ThetaData.
The palette, type, spacing, radii and action styling above are lifted
from the ThetaData UI foundations sheet (board F01 of the website
atlas), not invented here. Someone who has used the website should
recognise this as the same product family on first glance.

What the app adds is *density*. The brand sheet is sized for a marketing
page — 16px body, 48px controls, 64-96px section spacing. A downloader
is a working surface where a row of numbers has to stay readable
hundreds of rows deep, so the scale tightens: 14px body, 32px inputs,
13px tabular mono. The ratios, weights and colours are the brand's; the
sizes are the tool's.

Two instincts have to coexist:

1. **Trading-floor seriousness.** Tabular numbers in monospaced
   tabular-nums, dense information without visual stutter, predictable
   surfaces. A working tool, not a landing page.
2. **Browsing pleasure.** Featured shelves, dataset cards with spec
   previews, smooth detail pages, a queue that builds with tactile
   feedback.

## Color

Light is the default, because the brand is light. A dark theme ships
too — this app is a desktop tool people leave open next to a chart —
but it is the app's own variant, not a second identity: it keeps the
same blue rather than substituting a different hue.

Two blues, with different jobs, exactly as the brand sheet splits them:

- **Brand blue `#1074FF`** — identity. The logo, the active indicator,
  the focus ring. Fixed across both themes.
- **Action blue `#0063CC`** — interaction. Links and filled buttons.
  Hover and pressed *darken* on light; on a light canvas a lighter
  hover reads as disabled.

Everything else is greyscale. The eye should know that anything blue is
clickable and that nothing else is. Status colors (`good`, `warn`,
`bad`) appear only inside pills and badges, never as primary CTAs.

For market-data deltas inside dataset previews, use `data-bull` /
`data-bear` (green / red) — these are *data ink*, distinct from UI
status. Never use them on chrome.

Surfaces nest by one step of luminance: `bg` < `surface-1` <
`surface-2` < `surface-3`. A modal feels lifted because each surface is
one step from the one beneath; on light that step goes down in
lightness, on dark it goes up. Avoid drop shadows for elevation on
inline surfaces — the luminance step is the elevation.

**No colour literal belongs in a component.** Every wash, ring and pill
colour has a token (`--accent-tint`, `--good-ring`, `--tier-pro-bg`, …)
precisely because the first version of this app hard-coded rgba() into
29 files and could only ever be dark.

### Dark, and four directions that were rejected

The brand sheet is a light-mode document, so the dark variant had to be
derived rather than looked up. Four attempts were rejected in review;
they are recorded here so they are not retried.

- **A second hue as the dark accent — amber, then green.** Introducing
  any hue the brand does not own makes the dark theme read as a
  different product wearing the same logo. Purple was ruled out
  separately: it is close enough to Databento's identity to invite the
  comparison.
- **Cyan `#00D4FF` alongside the brand blue.** Turquoise and blue
  together at similar saturation fight rather than sit in a ladder.
- **Lightening the brand blue to `rgb(77, 155, 255)`.** It desaturates
  into a washed periwinkle that reads as disabled rather than
  interactive — the same failure as a lighter hover on a light canvas,
  a step up the page instead of into it.
- **A flat `rgb(11, 95, 208)` fill.** Legible, and inert. A large flat
  panel of one mid blue is also the worst possible bed for grey label
  text, which is where it was used.

What shipped instead: one hue, differentiated by **weight and
gradient** rather than by adding a colour. `--accent` stays the link
blue; `--accent-fill` is a gradient (`135deg`, `#0B63DD → #5B23D6`) for
filled actions, and `--brand-gradient` (`#1074FF → #00D4FF`) is allowed
only on surfaces that carry no text. The tier ladder climbs by weight,
with Pro as the only filled chip, which is why grey-on-fill never has
to be legible anywhere else.

The logo's own two values — `#a0a0a0` and `#1074FF` — are fixed in both
themes.

## Typography

Inter for everything UI, at the weights the brand sheet specifies (600
for display and headings, 500 for labels, 400 for body).
Tabular-nums on every number — counts, prices, sizes, rows, bytes,
dates. A row of `1,234,567` aligns digit-for-digit with the row above
and below, and that alignment is one of the things separating a data
tool from a generic dashboard.

`caption` (uppercase, tracked, muted) is for column headers, section
labels, and metadata pairs. Never for body copy.

For code-shaped values — tickers, contract specs
(`SPXW 20260516 P 5400`), file paths, hashes — use `--font-mono`, which
resolves to the platform's own modern mono: SF Mono on macOS, Cascadia
on Windows. The brand sheet names IBM Plex Mono, but that is a slab
face built for print and reads as a typewriter on screen; `ui-monospace`
also avoids shipping a webfont for a desktop app.

**A number in a column is not code.** Counts, byte sizes, dates and
percentages use `.tabnum` and `.text-figures`, both of which are the UI
face with tabular figures. Digits align perfectly, which is the actual
requirement, and the table stops reading as terminal output. Reach for
the mono only when the value would be wrong to re-wrap or re-space — a
path, a hash, a contract spec.

That distinction matters more than it sounds, because the generic
`monospace` keyword resolves to **Courier** on macOS. A stats line that
falls through to it looks like a 1984 dot-matrix report. The mono stack
therefore names real faces first — SF Mono, Cascadia, Menlo — and keeps
`monospace` only as the last resort it is.

**Inter is not installed on a stock macOS or Windows machine.** The
stack falls through to `system-ui`, which is SF Pro or Segoe UI, so the
app has been rendering in the system face rather than the brand one.
That is a perfectly good fallback and nothing looks broken, but the
brand sheet specifies Inter and the app does not currently honour it.
Bundling the variable font would fix it; until then this file should
not claim otherwise.

## The wordmark

`src/lib/brand/ThetaDataLogo.svelte` carries the shield paths from the
brand asset unaltered, with the "data" half of the wordmark set to
`currentColor` so one component serves both themes. The wordmark is
still a live `<text>` node rather than outlines; the brand sheet says
use the original SVG and never redraw it, so the outlined master should
replace this before the app is shown outside the team.

## Layout

Three primary regions:

1. **Top bar (48px).** Logo, global search, account / status. Drag
   region on desktop.
2. **Left rail (240px, collapsible to 64px).** Browse / Library / Queue /
   Settings. Below: the user's saved searches and pinned symbols.
3. **Content (fluid).** A single wide column. Internal layout is
   shelves (featured), grids (browse), or detail pages.

No floating panels, no resizable split panes. A linear, scrollable
content area is faster to grok and easier to ship across mac/win/linux.

## Information shelves (Browse)

The Browse view is a vertical stack of horizontally-scrolling shelves,
App-Store-style:

- **Featured today** — curated dataset bundles (e.g. "Mag 7 options last
  3y", "S&P 500 daily EOD 2024–2026").
- **Asset class** — one row each: Stocks, Options, Indices, Rates.
- **By cadence** — Trade tick, Quote tick, EOD, Snapshot, Greeks.
- **Index ecosystems** — S&P 500, NDX, Sp400, Sp600, Dji, Rut (sourced
  from `indexkit`, nightly fresh).
- **Recently updated** — datasets the user already has locally, sorted
  by last sync.

Each shelf is one viewport-tall maximum. Cards in a shelf are uniform
size; the shelf scrolls horizontally with momentum and a subtle gradient
fade on the right edge.

## Dataset card

The atomic unit. Used in shelves, grids, search results.

```
┌──────────────────────────────────────────┐
│ [icon]  ASSET CLASS · CADENCE            │
│                                          │
│  QQQ Option Trade-Quote                  │   ← display-lg
│  Every trade + paired NBBO quote         │   ← body
│                                          │
│  ─── tiny preview chart (opt) ───        │
│                                          │
│  3 yrs · 752 days · ~30 GB · parquet     │   ← caption mono
│                                          │
│  [+ Queue]                          ⋯    │
└──────────────────────────────────────────┘
```

The card has three states: **default**, **hover** (lifted to
surface-2 + accent border on action), **queued** (subtle accent glow on
the left edge, the "+ Queue" replaced with "✓ in queue"). Transition
between states uses `motion.duration.base` with `motion.easing.standard`.

## Dataset detail page

Clicking a card opens a full content page (not a modal — modals are for
forms only). Layout:

```
┌────────────────────────────────────────────────────────────┐
│ ← Back to Browse                                           │
│                                                            │
│  QQQ Option Trade-Quote                  [Add to Queue ▾]  │
│  Every trade + paired NBBO quote                           │
│                                                            │
│  ┌────────────────────────────────────────────────────┐    │
│  │ Schema · Sample · Coverage · Settings              │    │
│  └────────────────────────────────────────────────────┘    │
│  Tab content here. Default = Schema.                       │
└────────────────────────────────────────────────────────────┘
```

- **Schema** tab — column-by-column field table with type, nullable,
  description, example value. Same style as Hugging Face dataset
  schemas, applied to TradeQuoteTick / TradeTick / etc.
- **Sample** tab — a 50-row preview rendered as a virtualized table.
  Sourced from `endpoint_invoke` against a recent date.
- **Coverage** tab — calendar heatmap showing which dates the user
  already has locally (for this kind, this symbol) versus what's
  available upstream. Click a missing range to queue it.
- **Settings** tab — output format radio (parquet / csv / jsonl / json),
  output directory, default workers.

The "Add to Queue ▾" split button: primary action queues with last-used
defaults (last symbol picker, last date range). The dropdown opens an
inline composer to override before queueing.

## Queue (the user's "cart")

The queue reads like an inbox of work:

```
┌──────────────────────────────────────────────────────────────┐
│  Queue · 2,704 items · 11.4 GB est · ETA 3h 14m              │
│  [▶ Start workers]  [⏸ Pause]  [⟲ Retry failed (12)]         │
├──────────────────────────────────────────────────────────────┤
│  ● running  QQQ option_trade  2024-06-13   ▓▓▓▓▓▓░░  64 %    │
│  ● running  QQQ option_quote  2024-06-13   ▓▓░░░░░░  18 %    │
│  ○ pending  QQQ option_trade  2024-06-14                     │
│  ○ pending  QQQ option_quote  2024-06-14                     │
│  ○ pending  …                                                │
│  ✓ done     QQQ option_trade  2024-06-12   1.8 M rows · 41 MB│
│  ✕ failed   QQQ option_oi     2024-06-11   timeout           │
└──────────────────────────────────────────────────────────────┘
```

- Live progress bars on running rows, animated (180 ms updates).
- Hover any row to expose row-level actions: priority bump, cancel,
  duplicate, open file location.
- Status filter pills at the top: `all · pending · running · done ·
  failed · empty`.
- Aggregate header always visible: counts, est size, ETA derived from
  rolling avg of completed-task duration × pending count ÷ workers.

## Library

The "Datasets" tab from the prototype, redesigned as a search-first list:

```
┌───────────────────────────────────────────────────────┐
│ Library                          [⌕ filter symbol…]   │
├───────────────────────────────────────────────────────┤
│  QQQ                                                  │
│    option_trade        752 files  6.2 GB  2023..2026  │
│    option_quote        752 files  18.1 GB 2023..2026  │
│    stock_trade_quote   752 files  3.4 GB  2023..2026  │
│  SPY                                                  │
│    …                                                  │
└───────────────────────────────────────────────────────┘
```

Each row collapses on click to show a sparkline of bytes/rows over time,
and inline action buttons: re-run missing dates, open output dir, export
manifest, delete.

## Forms (for input)

Inputs are 32 px tall (compact-comfortable). All inputs share the same
visual: `surface-2` fill, 1 px `border` stroke, focus ring is 2 px
`accent` outline. No input has a colored fill on default state.

Multi-select uses chips that the user types-and-tabs into. Each chip is
removable. The chip background is `accent-tint`, the text is `accent-hi`.

Date inputs accept either a single day or a range; the picker is a
contemporary calendar with month-to-month animation matching
`motion.duration.base`.

The "New download" composer is **not** a modal anymore — it is an inline
popover anchored to the "+ Queue" button on a dataset card. Modals are
for confirmations, errors, and rare destructive actions only.

## Motion

Default transition: `120ms standard`. Page transitions: `260ms standard`,
slight x-slide on tab change. Numbers updating in place use a tween
(rolling counter) over 180 ms — never an instant repaint, which feels
janky on the queue header where bytes/rows change every second.

Hover lift on cards: `transform: translateY(-1px)` plus a luminance step
on background. No scale transforms — they cost subpixel sharpness on
text.

## Iconography

Lucide line icons throughout. Stroke 1.75 (slightly heavier than default
2 looks too busy at 14 px). Never emoji. Domain-specific glyphs (call /
put, bid / ask, expiration) are custom SVGs in `src/lib/icons/` that
match Lucide's style.

## Accessibility

Contrast: every fg-on-bg pair must hit WCAG AA (≥ 4.5:1) at body size,
AAA (≥ 7:1) for caption-on-card. `fg-muted` on `surface-1` measures
6.8:1. `fg` on `bg` measures 14.1:1.

Keyboard: every interactive element is reachable by tab; modals trap
focus; Escape closes overlays. Cmd/Ctrl-K opens global search.

## What this is not

- Not a torrent client. No retro-skeumorphic "queue manager" chrome.
- Not a dashboard. No glanceable hero numbers in the top corner.
- Not a webapp landing page. No marketing hero, no testimonials, no
  feature grid.
- Not Tailwind defaults. The palette is ThetaData's, not `slate-900`.
- Not a second brand. Nothing here invents a colour, a face or a radius
  the UI foundations sheet does not already specify.

The mental model is: the ThetaData website, compressed to working
density and given a queue.

---
name: TdDx Store
description: |
  TdDx Store is a desktop dataset marketplace for ThetaData market data.
  Users browse, preview, and download structured tick / quote / OHLC /
  greeks datasets across stocks, options, indices, and rates.
identity:
  archetype: "Premium financial data terminal × consumer dataset store"
  references:
    - Bloomberg Terminal (information density, monospaced precision)
    - Hugging Face Datasets (browse → preview → download flow)
    - Linear (motion, restraint, surface hierarchy)
    - Apple App Store (featured shelves, detail pages, cart)
colors:
  bg: "#0B0E14"          # Near-black, slightly cool. Surface 0.
  surface-1: "#11151D"   # Cards, sidebars.
  surface-2: "#161B25"   # Hover, raised cards.
  surface-3: "#1E2430"   # Modals, popovers.
  border: "#222936"      # Hairlines.
  border-strong: "#2A3242"
  fg: "#E6EAF2"          # Primary text.
  fg-muted: "#9AA3B2"    # Secondary text, captions.
  fg-subtle: "#5C6577"   # Disabled, placeholders.
  accent: "#7C8CFF"      # Periwinkle indigo. Sole driver of interaction.
  accent-hi: "#9AA8FF"   # Hover.
  accent-lo: "#5868D9"   # Pressed.
  accent-tint: "#7C8CFF1F"  # 12% accent for backgrounds.
  good: "#5DD4A0"
  warn: "#F5C56F"
  bad: "#FF7E7E"
  data-bull: "#34D399"   # Green for up moves / positive deltas.
  data-bear: "#F87171"   # Red for down moves.
typography:
  display-xl:
    fontFamily: "Inter Display, Inter, system-ui"
    fontSize: "2.5rem"
    fontWeight: 600
    letterSpacing: "-0.02em"
    lineHeight: 1.1
  display-lg:
    fontFamily: "Inter Display, Inter, system-ui"
    fontSize: "1.75rem"
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
    fontFamily: "JetBrains Mono, SF Mono, Consolas, ui-monospace, monospace"
    fontSize: "0.8125rem"
    fontVariantNumeric: "tabular-nums"
    fontWeight: 400
    lineHeight: 1.4
rounded:
  none: "0px"
  sm: "6px"
  md: "10px"
  lg: "14px"
  xl: "20px"
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
shadows:
  flat: "0 0 0 1px rgba(0,0,0,0.2)"
  raised: "0 1px 2px rgba(0,0,0,0.4), 0 0 0 1px rgba(0,0,0,0.25)"
  modal: "0 24px 64px rgba(0,0,0,0.55), 0 0 0 1px rgba(0,0,0,0.4)"
  glow-accent: "0 0 0 4px rgba(124,140,255,0.18)"
motion:
  duration:
    instant: "60ms"
    fast: "120ms"
    base: "180ms"
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
  stroke: 1.75
---

## Identity

TdDx Store is a *dataset marketplace for traders and quants* — a place to
shop for high-fidelity market data the way someone shops the App Store
for an app. The aesthetic must reconcile two opposing instincts:

1. **Trading-floor seriousness.** Tabular numbers in monospaced
   tabular-nums, dense information without visual stutter, predictable
   surfaces. A working tool, not a landing page.
2. **Consumer browsing pleasure.** Featured shelves, dataset cards with
   spec previews, smooth detail pages, a queue ("cart") that builds with
   tactile feedback.

The result reads as a premium financial terminal that has been re-skinned
by a contemporary product team. Serious where it must be (the data),
delightful where it should be (the browsing).

## Color

Single accent (periwinkle indigo `#7C8CFF`) is the only interactive color.
All other ink is grayscale. This is non-negotiable: the eye should know
that anything indigo is clickable, and that nothing else is. Status
colors (`good`, `warn`, `bad`) appear only inside pills and badges,
never as primary CTAs.

For market-data deltas inside dataset previews, use `data-bull` /
`data-bear` (green / red) — these are *data ink*, distinct from UI
status. Never use them on chrome.

Surfaces nest by 0.5–0.7 units of luminance: `bg` < `surface-1` <
`surface-2` < `surface-3`. A modal feels lifted because each surface is
one step brighter than the one beneath. Avoid drop shadows for elevation
on inline surfaces — the luminance step is the elevation.

## Typography

Inter for everything UI. Inter Display for any text ≥ 1.5rem (large
headings). Tabular-nums on every number — counts, prices, sizes, rows,
bytes, dates. A row of `1,234,567` aligns digit-for-digit with the row
above and below, and that alignment is one of the things separating a
data tool from a generic dashboard.

`caption` (uppercase, tracked, muted) is for column headers, section
labels, and metadata pairs. Never for body copy.

JetBrains Mono for: tickers, contract specs (`SPXW 20260516 P 5400`),
file paths, hashes, timestamps, code-like values.

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

The queue is presented like a shopping basket / Linear inbox:

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
- Not Tailwind defaults. The palette is hand-tuned, not `slate-900`.

The mental model is: Bloomberg Terminal, redesigned by Linear, with the
content layer of a modern dataset marketplace.

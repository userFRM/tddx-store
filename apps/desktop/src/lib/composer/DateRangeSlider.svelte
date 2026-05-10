<script lang="ts">
  /**
   * Visual date range slider over the days the server actually has data
   * for a given symbol + kind. Two draggable cursors set start / end;
   * a tick row underneath shows the available-day density. Read-only
   * unless `availableDates` is non-empty.
   *
   * Inputs:
   *   bind:start  — YYYY-MM-DD or YYYYMMDD
   *   bind:end    — YYYY-MM-DD or YYYYMMDD
   *   availableDates: NaiveDate-like strings sorted ascending. The slider
   *                   maps cursor position to indices into this array.
   *
   * If `availableDates` is empty, the component falls back to a plain
   * date-range pair of inputs so the user can still type values.
   */
  import { onMount } from "svelte";

  let {
    start = $bindable(""),
    end = $bindable(""),
    availableDates = [] as string[],
    label = "Date range",
  }: {
    start: string;
    end: string;
    availableDates?: string[];
    label?: string;
  } = $props();

  // ── Helpers ───────────────────────────────────────────────
  function normaliseDate(s: string): string {
    // Accept YYYY-MM-DD, YYYYMMDD, etc — return YYYY-MM-DD.
    if (!s) return "";
    const t = s.replace(/-/g, "");
    if (t.length !== 8) return s;
    return `${t.slice(0, 4)}-${t.slice(4, 6)}-${t.slice(6, 8)}`;
  }
  function ymd8(s: string): string {
    return s.replace(/-/g, "");
  }
  const sortedDates = $derived(
    availableDates
      .map(normaliseDate)
      .filter(d => /^\d{4}-\d{2}-\d{2}$/.test(d))
      .sort(),
  );
  const haveSlider = $derived(sortedDates.length > 1);

  // Index of a date in sortedDates; -1 if not present. Snap to nearest.
  function indexOfNearest(d: string): number {
    if (!sortedDates.length) return -1;
    const target = normaliseDate(d);
    let lo = 0, hi = sortedDates.length - 1;
    while (lo < hi) {
      const m = (lo + hi) >> 1;
      if (sortedDates[m] < target) lo = m + 1;
      else hi = m;
    }
    return lo;
  }

  // ── Cursor positions as indices into sortedDates ────────────
  let trackEl = $state<HTMLDivElement | null>(null);
  let dragging = $state<"start" | "end" | null>(null);
  let startIdx = $state(0);
  let endIdx = $state(0);

  // Initialise positions when the available dates resolve.
  $effect(() => {
    if (!haveSlider) return;
    const last = sortedDates.length - 1;
    if (start) {
      startIdx = Math.max(0, Math.min(last, indexOfNearest(start)));
    } else {
      startIdx = Math.max(0, last - 30);
    }
    if (end) {
      endIdx = Math.max(0, Math.min(last, indexOfNearest(end)));
    } else {
      endIdx = last;
    }
    if (endIdx < startIdx) endIdx = startIdx;
    syncBindings();
  });

  function syncBindings() {
    if (!sortedDates.length) return;
    start = sortedDates[startIdx] ?? start;
    end = sortedDates[endIdx] ?? end;
  }

  function pctOfIdx(i: number): number {
    const last = sortedDates.length - 1;
    if (last <= 0) return 0;
    return (i / last) * 100;
  }

  function pointerToIdx(clientX: number): number {
    if (!trackEl) return 0;
    const r = trackEl.getBoundingClientRect();
    const last = sortedDates.length - 1;
    const t = (clientX - r.left) / Math.max(r.width, 1);
    const clamped = Math.max(0, Math.min(1, t));
    return Math.round(clamped * last);
  }

  function onPointerDown(which: "start" | "end", e: PointerEvent) {
    e.preventDefault();
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    dragging = which;
  }
  function onPointerMove(e: PointerEvent) {
    if (!dragging) return;
    const i = pointerToIdx(e.clientX);
    if (dragging === "start") {
      startIdx = Math.min(i, endIdx);
    } else {
      endIdx = Math.max(i, startIdx);
    }
    syncBindings();
  }
  function onPointerUp(e: PointerEvent) {
    if (!dragging) return;
    (e.target as HTMLElement).releasePointerCapture?.(e.pointerId);
    dragging = null;
  }
  function onTrackClick(e: MouseEvent) {
    const i = pointerToIdx(e.clientX);
    // Move the nearest cursor.
    if (Math.abs(i - startIdx) <= Math.abs(i - endIdx)) startIdx = Math.min(i, endIdx);
    else endIdx = Math.max(i, startIdx);
    syncBindings();
  }
  function onKey(which: "start" | "end", e: KeyboardEvent) {
    let delta = 0;
    if (e.key === "ArrowLeft")  delta = -1;
    if (e.key === "ArrowRight") delta = +1;
    if (e.key === "Home")       delta = -1e9;
    if (e.key === "End")        delta = +1e9;
    if (e.shiftKey) delta *= 7;
    if (delta === 0) return;
    e.preventDefault();
    const last = sortedDates.length - 1;
    if (which === "start") {
      startIdx = Math.max(0, Math.min(endIdx, startIdx + delta));
    } else {
      endIdx = Math.max(startIdx, Math.min(last, endIdx + delta));
    }
    syncBindings();
  }

  const rangeDays = $derived(
    haveSlider ? endIdx - startIdx + 1 : 0,
  );
  const fmtBucket = (n: number) => n.toLocaleString();
</script>

<div class="dr-wrap">
  <div class="dr-head">
    <span class="text-caption">{label}</span>
    {#if haveSlider}
      <span class="dr-summary tabnum">
        <span class="dr-pill">{sortedDates[startIdx]}</span>
        <span class="dr-arrow">→</span>
        <span class="dr-pill">{sortedDates[endIdx]}</span>
        <span class="dr-meta">·</span>
        <span class="dr-meta">{fmtBucket(rangeDays)} trading day{rangeDays === 1 ? "" : "s"}</span>
        <span class="dr-meta">·</span>
        <span class="dr-meta">{fmtBucket(sortedDates.length)} available</span>
      </span>
    {/if}
  </div>

  {#if haveSlider}
    <div class="dr-slider"
         role="group"
         aria-label="Date range"
         onpointermove={onPointerMove}
         onpointerup={onPointerUp}
         onpointercancel={onPointerUp}>
      <div class="dr-track" bind:this={trackEl} onclick={onTrackClick}>
        <div class="dr-track-bg"></div>
        <div class="dr-track-fill"
             style:left="{pctOfIdx(startIdx)}%"
             style:right="{100 - pctOfIdx(endIdx)}%"></div>

        <!-- Start handle -->
        <button
          type="button"
          class="dr-handle"
          class:dragging={dragging === "start"}
          style:left="{pctOfIdx(startIdx)}%"
          onpointerdown={(e) => onPointerDown("start", e)}
          onkeydown={(e) => onKey("start", e)}
          aria-label="Start date"
          aria-valuemin="0"
          aria-valuemax={sortedDates.length - 1}
          aria-valuenow={startIdx}
          aria-valuetext={sortedDates[startIdx]}
        ></button>

        <!-- End handle -->
        <button
          type="button"
          class="dr-handle"
          class:dragging={dragging === "end"}
          style:left="{pctOfIdx(endIdx)}%"
          onpointerdown={(e) => onPointerDown("end", e)}
          onkeydown={(e) => onKey("end", e)}
          aria-label="End date"
          aria-valuemin="0"
          aria-valuemax={sortedDates.length - 1}
          aria-valuenow={endIdx}
          aria-valuetext={sortedDates[endIdx]}
        ></button>
      </div>
      <div class="dr-anchors tabnum">
        <span>{sortedDates[0]}</span>
        <span>{sortedDates[sortedDates.length - 1]}</span>
      </div>
    </div>
  {:else}
    <div class="dr-fallback">
      <label class="field-stack">
        <span class="text-caption">Start</span>
        <input class="field-input" bind:value={start} placeholder="2024-01-01" />
      </label>
      <label class="field-stack">
        <span class="text-caption">End</span>
        <input class="field-input" bind:value={end} placeholder="2026-05-08" />
      </label>
    </div>
    <p class="dr-fallback-hint fg-muted text-body-sm">
      Connect to ThetaData &amp; pick a symbol to see the live trading-day
      slider here.
    </p>
  {/if}
</div>

<style>
  .dr-wrap { display: flex; flex-direction: column; gap: var(--sp-2); }

  .dr-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--sp-3);
  }
  .dr-summary {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--text-body-sm);
  }
  .dr-pill {
    background: var(--surface-3);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 1px var(--sp-2);
    color: var(--fg);
    font-family: var(--font-mono);
  }
  .dr-arrow { color: var(--fg-muted); font-size: 12px; }
  .dr-meta { color: var(--fg-muted); font-family: var(--font-ui); }

  .dr-slider {
    user-select: none;
    padding: var(--sp-3) var(--sp-1) var(--sp-2);
  }
  .dr-track {
    position: relative;
    height: 6px;
    cursor: pointer;
  }
  .dr-track-bg {
    position: absolute;
    inset: 0;
    background: var(--surface-3);
    border-radius: var(--r-pill);
  }
  .dr-track-fill {
    position: absolute;
    top: 0; bottom: 0;
    background: linear-gradient(90deg, var(--accent), var(--accent-hi));
    border-radius: var(--r-pill);
  }

  .dr-handle {
    position: absolute;
    top: 50%;
    width: 16px;
    height: 16px;
    margin-left: -8px;
    transform: translateY(-50%);
    background: var(--fg);
    border: 2px solid var(--accent);
    border-radius: 50%;
    cursor: grab;
    padding: 0;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.5);
    transition: transform var(--dur-fast) var(--ease-standard),
                box-shadow var(--dur-fast) var(--ease-standard);
  }
  .dr-handle:hover {
    transform: translateY(-50%) scale(1.12);
  }
  .dr-handle:focus-visible {
    outline: none;
    box-shadow: var(--shadow-glow-accent), 0 1px 4px rgba(0, 0, 0, 0.5);
  }
  .dr-handle.dragging {
    cursor: grabbing;
    transform: translateY(-50%) scale(1.18);
  }

  .dr-anchors {
    display: flex;
    justify-content: space-between;
    margin-top: var(--sp-2);
    font-size: var(--text-caption);
    color: var(--fg-subtle);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }

  .dr-fallback {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-3);
  }
  .dr-fallback-hint { margin-top: var(--sp-2); }
  .field-stack { display: flex; flex-direction: column; gap: 4px; }
</style>

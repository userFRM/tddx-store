<script lang="ts">
  /**
   * One downloaded file, drawn so its completeness is obvious.
   *
   * Deliberately one question per dataset and nothing more: *did I get
   * what I asked for?* No indicators, overlays or drawing tools — that
   * is a terminal, not a downloader.
   *
   * Candles are spaced evenly, one slot each, the way trading charts
   * draw them. Placed by time, a week's five days bunched together with
   * a weekend-shaped hole beside them, and a day's session left most of
   * the width to the night. How long each candle spans is chosen to fit
   * the width — Auto — and can be changed; the backend offers only the
   * intervals the file can honestly be drawn at.
   */
  import { Loader2, AlertTriangle, CheckCircle2 } from "lucide-svelte";
  import { api, fmtNum, type ChartSeries } from "$lib/api";

  let { path }: { path: string } = $props();

  let data = $state<ChartSeries | null>(null);
  let error = $state("");
  let loading = $state(false);
  /** `null` is Auto: the finest interval that fits the width. */
  let step = $state<string | null>(null);
  let width = $state(0);
  /** Room for the price axis on the right. */
  const PAD_R = 64;

  /** About ten pixels a candle: a body you can see, and air between. */
  const target = $derived(Math.max(20, Math.min(400, Math.floor((width - PAD_R) / 10))));
  /** The last request made. Deliberately not reactive: reading `data`
   *  here to decide would make every response trigger the next fetch. */
  let lastKey = "";

  $effect(() => {
    const p = path;
    const st = step;
    if (!p || width === 0) return;
    const t = target;
    // Auto refetches only when the width moves far enough to change the
    // answer — resizing a window a few pixels should not reread a file.
    const band = st === null ? Math.round(Math.log(t) / Math.log(1.25)) : 0;
    const key = `${p}|${st}|${band}`;
    if (key === lastKey) return;
    lastKey = key;
    let cancelled = false;
    loading = true;
    error = "";
    api
      .chartSeries(p, st, t)
      .then((d) => !cancelled && (data = d))
      .catch((e: unknown) => !cancelled && (error = e instanceof Error ? e.message : String(e)))
      .finally(() => !cancelled && (loading = false));
    return () => {
      cancelled = true;
    };
  });

  // A different file starts from Auto.
  $effect(() => {
    void path;
    step = null;
  });

  // ── Geometry: drawn 1:1 in pixels, so text is never stretched ──
  const PRICE_H = 300;
  const VOL_H = 70;
  const GAP_Y = 12;
  const H = PRICE_H + GAP_Y + VOL_H;
  const W = $derived(Math.max(320, width));
  const PLOT_W = $derived(W - PAD_R);

  const n = $derived(data?.x.length ?? 0);
  const slot = $derived(n ? PLOT_W / n : 1);
  const bodyW = $derived(Math.max(1, Math.min(14, slot * 0.66)));
  const cx = (i: number) => (i + 0.5) * slot;

  const bounds = $derived.by(() => {
    if (!data || n === 0) return null;
    let lo = Infinity;
    let hi = -Infinity;
    for (const arr of [data.low, data.high, data.bid, data.ask]) {
      for (const v of arr) {
        if (v === null) continue;
        if (v < lo) lo = v;
        if (v > hi) hi = v;
      }
    }
    if (!isFinite(lo)) return null;
    const pad = (hi - lo || hi * 0.01 || 1) * 0.06;
    let vmax = 1;
    for (const v of data.volume) if (v !== null && v > vmax) vmax = v;
    return { lo: lo - pad, hi: hi + pad, vmax };
  });

  const sy = (v: number) => (bounds ? PRICE_H - ((v - bounds.lo) / (bounds.hi - bounds.lo)) * PRICE_H : 0);
  const sv = (v: number) => (bounds ? (v / bounds.vmax) * VOL_H : 0);

  function line(ys: (number | null)[]): string {
    let d = "";
    let pen = false;
    ys.forEach((v, i) => {
      if (v === null) {
        pen = false;
        return;
      }
      d += `${pen ? "L" : "M"}${cx(i).toFixed(1)},${sy(v).toFixed(1)}`;
      pen = true;
    });
    return d;
  }

  /** A filled area between two series, broken wherever either is null. */
  function area(top: (number | null)[], bottom: (number | null)[]): string {
    const runs: number[][] = [];
    let run: number[] = [];
    top.forEach((_, i) => {
      if (top[i] === null || bottom[i] === null) {
        if (run.length) runs.push(run);
        run = [];
      } else run.push(i);
    });
    if (run.length) runs.push(run);
    return runs
      .filter((r) => r.length > 1)
      .map((r) => {
        const up = r.map((i) => `${cx(i).toFixed(1)},${sy(top[i]!).toFixed(1)}`);
        const down = [...r].reverse().map((i) => `${cx(i).toFixed(1)},${sy(bottom[i]!).toFixed(1)}`);
        return `M${up.join("L")}L${down.join("L")}Z`;
      })
      .join(" ");
  }

  const hasQuotes = $derived(!!data && data.bid.some((v) => v !== null) && data.ask.some((v) => v !== null));

  /** With candles evenly spaced, a gap has no width of its own; mark
   *  the boundary between the two candles it falls between. */
  const gapMarks = $derived.by(() => {
    if (!data) return [];
    return data.gaps.map((g) => {
      let i = 0;
      while (i < n - 1 && data!.x[i + 1] <= g.from_ms) i++;
      return { x: (i + 1) * slot, key: g.from_ms };
    });
  });

  // ── Axes ──────────────────────────────────────────────────────
  const multiDay = $derived(!!data && n > 1 && data.x[n - 1] - data.x[0] >= 86_400_000);
  function fmtTime(ms: number): string {
    const iso = new Date(ms).toISOString();
    if (!data || data.daily) return iso.slice(0, 10);
    return multiDay ? `${iso.slice(5, 10)} ${iso.slice(11, 16)}` : iso.slice(11, 16);
  }
  const xTicks = $derived.by(() => {
    if (!data || n === 0) return [];
    const count = Math.min(6, n);
    return Array.from({ length: count }, (_, k) => {
      const i = count === 1 ? 0 : Math.round((k * (n - 1)) / (count - 1));
      return { x: cx(i), label: fmtTime(data!.x[i]), first: k === 0, last: k === count - 1 };
    });
  });
  const yTicks = $derived.by(() => {
    if (!bounds) return [];
    return Array.from({ length: 5 }, (_, k) => {
      const v = bounds.lo + ((bounds.hi - bounds.lo) * k) / 4;
      return { y: sy(v), label: v >= 100 ? v.toFixed(2) : v.toPrecision(4) };
    });
  });

  // ── Hover ─────────────────────────────────────────────────────
  let hover = $state<number | null>(null);
  let svgEl = $state<SVGSVGElement | null>(null);

  function onMove(e: PointerEvent) {
    if (!svgEl || n === 0) return;
    const px = e.clientX - svgEl.getBoundingClientRect().left;
    hover = px > PLOT_W || px < 0 ? null : Math.min(n - 1, Math.floor(px / slot));
  }

  function fmtGap(g: { from_ms: number; to_ms: number }): string {
    const mins = Math.round((g.to_ms - g.from_ms) / 60_000);
    const span =
      data?.daily ? `${Math.round(mins / 1440)} days`
      : mins >= 60 ? `${Math.floor(mins / 60)}h ${mins % 60}m`
      : `${mins}m`;
    return `${fmtTime(g.from_ms)} → ${fmtTime(g.to_ms)} · ${span} with no rows`;
  }
</script>

<div class="chart" bind:clientWidth={width}>
  {#if data && data.steps.length > 1}
    <div class="intervals" role="radiogroup" aria-label="Candle interval">
      <button role="radio" aria-checked={step === null} class:active={step === null} onclick={() => (step = null)}>
        Auto{step === null && data ? ` · ${data.step}` : ""}
      </button>
      {#each data.steps as s (s)}
        <button role="radio" aria-checked={step === s} class:active={step === s} onclick={() => (step = s)}>{s}</button>
      {/each}
    </div>
  {/if}

  {#if error}
    <div class="state bad"><AlertTriangle size={16} /> {error}</div>
  {:else if !data || !bounds}
    <div class="state"><Loader2 size={16} class="spin" /> Reading the file…</div>
  {:else}
    <svg
      bind:this={svgEl}
      width={W}
      height={H + 18}
      viewBox="0 0 {W} {H + 18}"
      class:dim={loading}
      onpointermove={onMove}
      onpointerleave={() => (hover = null)}
      role="img"
      aria-label="Chart of {fmtNum(data.rows)} rows in {data.step} candles"
    >
      {#each yTicks as t (t.y)}
        <line class="grid" x1="0" x2={PLOT_W} y1={t.y} y2={t.y} />
        <text class="axis" x={PLOT_W + 6} y={Math.min(PRICE_H, Math.max(11, t.y + 4))}>{t.label}</text>
      {/each}

      {#each gapMarks as g (g.key)}
        <line class="gap" x1={g.x} x2={g.x} y1="0" y2={H} />
      {/each}

      {#if hasQuotes}
        <path class="quote-band" d={area(data.ask, data.bid)} />
      {/if}

      {#if data.shape === "candles"}
        {#each data.x as x, i (x)}
          {#if data.high[i] !== null && data.low[i] !== null}
            {@const up = (data.close[i] ?? 0) >= (data.open[i] ?? 0)}
            <line class="wick" class:up class:down={!up} x1={cx(i)} x2={cx(i)} y1={sy(data.high[i]!)} y2={sy(data.low[i]!)} />
            {#if data.open[i] !== null && data.close[i] !== null}
              <rect
                class="body" class:up class:down={!up}
                x={cx(i) - bodyW / 2}
                y={Math.min(sy(data.open[i]!), sy(data.close[i]!))}
                width={bodyW}
                height={Math.max(1, Math.abs(sy(data.open[i]!) - sy(data.close[i]!)))}
              />
            {/if}
          {/if}
        {/each}
      {:else if data.shape === "line"}
        <path class="envelope" d={area(data.high, data.low)} />
        <path class="price" d={line(data.close)} />
      {:else}
        <path class="price" d={line(data.bid.map((b, i) => (b !== null && data!.ask[i] !== null ? (b + data!.ask[i]!) / 2 : null)))} />
      {/if}

      <g transform="translate(0,{PRICE_H + GAP_Y})">
        {#each data.x as x, i (x)}
          {#if data.volume[i]}
            <rect class="vol" x={cx(i) - bodyW / 2} y={VOL_H - sv(data.volume[i]!)} width={bodyW} height={sv(data.volume[i]!)} />
          {/if}
        {/each}
      </g>

      {#each xTicks as t (t.x)}
        <text class="axis" x={t.x} y={H + 14} text-anchor={t.first ? "start" : t.last ? "end" : "middle"}>{t.label}</text>
      {/each}

      {#if hover !== null}
        <line class="cross" x1={cx(hover)} x2={cx(hover)} y1="0" y2={H} />
      {/if}
    </svg>

    <div class="readout tabnum">
      {#if hover !== null}
        {@const i = hover}
        <span class="fg-muted">{fmtTime(data.x[i])}</span>
        {#if data.shape === "candles"}
          <span>O {data.open[i]?.toFixed(2) ?? "—"}</span>
          <span>H {data.high[i]?.toFixed(2) ?? "—"}</span>
          <span>L {data.low[i]?.toFixed(2) ?? "—"}</span>
          <span>C {data.close[i]?.toFixed(2) ?? "—"}</span>
        {:else if data.shape === "line"}
          <span>{data.close[i]?.toFixed(2) ?? "—"}</span>
        {/if}
        {#if data.bid[i] !== null}<span>Bid {data.bid[i]!.toFixed(2)}</span>{/if}
        {#if data.ask[i] !== null}<span>Ask {data.ask[i]!.toFixed(2)}</span>{/if}
        {#if data.volume[i]}<span class="fg-muted">Vol {fmtNum(data.volume[i]!)}</span>{/if}
      {:else}
        <span class="fg-muted">
          {fmtNum(data.rows)} rows as {fmtNum(n)} {data.step} candle{n === 1 ? "" : "s"} · hover for values
        </span>
      {/if}
    </div>

    {#if data.gaps.length}
      <ul class="gaps">
        {#each data.gaps as g (g.from_ms)}
          <li><AlertTriangle size={13} /> {fmtGap(g)}</li>
        {/each}
      </ul>
    {:else}
      <p class="complete"><CheckCircle2 size={13} /> No gaps — rows arrive at a steady rhythm from start to end.</p>
    {/if}
  {/if}
</div>

<style>
  .chart { display: flex; flex-direction: column; gap: var(--sp-3); padding: var(--sp-4) var(--sp-5); }
  svg { display: block; touch-action: none; transition: opacity var(--dur-fast) var(--ease-standard); }
  svg.dim { opacity: 0.5; }
  .intervals {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 2px;
    align-self: flex-start;
    padding: 2px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
  }
  .intervals button {
    border: none;
    background: none;
    padding: 3px 9px;
    border-radius: calc(var(--r-sm) - 2px);
    font: inherit;
    font-size: var(--text-caption);
    font-family: var(--font-numeric);
    color: var(--fg-muted);
    cursor: pointer;
  }
  .intervals button.active { background: var(--surface-3); color: var(--fg); }
  .grid { stroke: var(--border); stroke-width: 1; vector-effect: non-scaling-stroke; }
  .axis { fill: var(--fg-subtle); font-size: 11px; font-family: var(--font-numeric); }
  .gap { stroke: var(--bad); stroke-width: 2; stroke-dasharray: 4 3; opacity: 0.7; }
  .quote-band { fill: var(--accent); opacity: 0.12; }
  .envelope { fill: var(--accent); opacity: 0.14; }
  .price { fill: none; stroke: var(--accent); stroke-width: 1.5; vector-effect: non-scaling-stroke; }
  .wick { stroke-width: 1; vector-effect: non-scaling-stroke; }
  .wick.up, .body.up { stroke: var(--data-bull); fill: var(--data-bull); }
  .wick.down, .body.down { stroke: var(--data-bear); fill: var(--data-bear); }
  .vol { fill: var(--fg-subtle); opacity: 0.35; }
  .cross { stroke: var(--fg-muted); stroke-width: 1; stroke-dasharray: 3 3; vector-effect: non-scaling-stroke; }
  .readout { display: flex; flex-wrap: wrap; gap: var(--sp-3); font-size: var(--text-body-sm); min-height: 1.4em; }
  .gaps { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; }
  .gaps li, .complete {
    display: flex; align-items: center; gap: 6px;
    font-size: var(--text-body-sm); margin: 0;
  }
  .gaps li { color: var(--bad); }
  .complete { color: var(--good); }
  .state {
    display: flex; align-items: center; gap: 8px; justify-content: center;
    min-height: 200px; color: var(--fg-muted);
  }
  .state.bad { color: var(--bad); }
</style>

<script lang="ts">
  /**
   * One downloaded file, drawn so its completeness is obvious.
   *
   * Deliberately one question per dataset and nothing more: *did I get
   * what I asked for?* A trade file with a two-hour hole looks identical
   * to a complete one in a row table, and obvious here. No indicators,
   * overlays or drawing tools — that is a terminal, not a downloader.
   *
   * The chart follows the columns (see `tdds_core::chart`): candles for
   * OHLC, a line inside its high–low envelope for a price, a band for a
   * bid and ask. Gaps the file has, relative to its own rhythm, are
   * shaded and listed underneath.
   */
  import { Loader2, AlertTriangle, CheckCircle2 } from "lucide-svelte";
  import { api, fmtNum, type ChartSeries } from "$lib/api";

  let { path }: { path: string } = $props();

  let data = $state<ChartSeries | null>(null);
  let error = $state("");
  let loading = $state(false);

  $effect(() => {
    const p = path;
    if (!p) return;
    let cancelled = false;
    loading = true;
    error = "";
    data = null;
    api
      .chartSeries(p)
      .then((d) => !cancelled && (data = d))
      .catch((e: unknown) => !cancelled && (error = e instanceof Error ? e.message : String(e)))
      .finally(() => !cancelled && (loading = false));
    return () => {
      cancelled = true;
    };
  });

  // ── Geometry ──────────────────────────────────────────────────
  const W = 1000;
  const PRICE_H = 300;
  const VOL_H = 70;
  const GAP_Y = 12;
  const H = PRICE_H + GAP_Y + VOL_H;
  const PAD_R = 64;
  const PLOT_W = W - PAD_R;

  const bounds = $derived.by(() => {
    if (!data || data.x.length === 0) return null;
    const vals: number[] = [];
    for (const arr of [data.low, data.high, data.bid, data.ask]) {
      for (const v of arr) if (v !== null) vals.push(v);
    }
    if (!vals.length) return null;
    let lo = Math.min(...vals);
    let hi = Math.max(...vals);
    const pad = (hi - lo || hi * 0.01 || 1) * 0.06;
    lo -= pad;
    hi += pad;
    const t0 = data.x[0];
    const t1 = data.x[data.x.length - 1];
    const vmax = Math.max(1, ...data.volume.map((v) => v ?? 0));
    return { lo, hi, t0, t1: t1 === t0 ? t0 + 1 : t1, vmax };
  });

  const sx = (t: number) => (bounds ? ((t - bounds.t0) / (bounds.t1 - bounds.t0)) * PLOT_W : 0);
  const sy = (v: number) => (bounds ? PRICE_H - ((v - bounds.lo) / (bounds.hi - bounds.lo)) * PRICE_H : 0);
  const sv = (v: number) => (bounds ? (v / bounds.vmax) * VOL_H : 0);

  const slot = $derived(data && data.x.length ? PLOT_W / data.x.length : 1);
  const bodyW = $derived(Math.max(1, Math.min(10, slot * 0.7)));

  function path_(ys: (number | null)[]): string {
    if (!data) return "";
    let d = "";
    let pen = false;
    ys.forEach((v, i) => {
      if (v === null) {
        pen = false;
        return;
      }
      d += `${pen ? "L" : "M"}${sx(data!.x[i]).toFixed(1)},${sy(v).toFixed(1)}`;
      pen = true;
    });
    return d;
  }

  /** A filled area between two series, broken wherever either is null. */
  function area(top: (number | null)[], bottom: (number | null)[]): string {
    if (!data) return "";
    const runs: number[][] = [];
    let run: number[] = [];
    data.x.forEach((_, i) => {
      if (top[i] === null || bottom[i] === null) {
        if (run.length) runs.push(run);
        run = [];
      } else run.push(i);
    });
    if (run.length) runs.push(run);
    return runs
      .filter((r) => r.length > 1)
      .map((r) => {
        const up = r.map((i) => `${sx(data!.x[i]).toFixed(1)},${sy(top[i]!).toFixed(1)}`);
        const down = [...r].reverse().map((i) => `${sx(data!.x[i]).toFixed(1)},${sy(bottom[i]!).toFixed(1)}`);
        return `M${up.join("L")}L${down.join("L")}Z`;
      })
      .join(" ");
  }

  const hasQuotes = $derived(!!data && data.bid.some((v) => v !== null) && data.ask.some((v) => v !== null));

  // ── Axes ──────────────────────────────────────────────────────
  function fmtTime(ms: number, daily: boolean): string {
    const d = new Date(ms);
    const iso = d.toISOString();
    return daily ? iso.slice(0, 10) : iso.slice(11, 16);
  }
  const xTicks = $derived.by(() => {
    if (!bounds || !data) return [];
    return Array.from({ length: 5 }, (_, k) => {
      const t = bounds.t0 + ((bounds.t1 - bounds.t0) * k) / 4;
      return { x: sx(t), label: fmtTime(t, data!.daily) };
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
    if (!data || !svgEl || !bounds) return;
    const r = svgEl.getBoundingClientRect();
    const px = ((e.clientX - r.left) / r.width) * W;
    if (px > PLOT_W) {
      hover = null;
      return;
    }
    const t = bounds.t0 + (px / PLOT_W) * (bounds.t1 - bounds.t0);
    let best = 0;
    let bestD = Infinity;
    data.x.forEach((x, i) => {
      const d = Math.abs(x - t);
      if (d < bestD) {
        bestD = d;
        best = i;
      }
    });
    hover = best;
  }

  function fmtGap(g: { from_ms: number; to_ms: number }, daily: boolean): string {
    const mins = Math.round((g.to_ms - g.from_ms) / 60_000);
    const span =
      daily ? `${Math.round(mins / 1440)} days`
      : mins >= 60 ? `${Math.floor(mins / 60)}h ${mins % 60}m`
      : `${mins}m`;
    return `${fmtTime(g.from_ms, daily)} → ${fmtTime(g.to_ms, daily)} · ${span} with no rows`;
  }
</script>

{#if loading}
  <div class="state"><Loader2 size={16} class="spin" /> Reading the file…</div>
{:else if error}
  <div class="state bad"><AlertTriangle size={16} /> {error}</div>
{:else if data && bounds}
  <div class="chart">
    <svg
      bind:this={svgEl}
      viewBox="0 0 {W} {H + 18}"
      preserveAspectRatio="none"
      onpointermove={onMove}
      onpointerleave={() => (hover = null)}
      role="img"
      aria-label="Chart of {fmtNum(data.rows)} rows"
    >
      {#each yTicks as t (t.y)}
        <line class="grid" x1="0" x2={PLOT_W} y1={t.y} y2={t.y} />
        <!-- Kept inside the frame: the top tick sits on the edge. -->
        <text class="axis" x={PLOT_W + 6} y={Math.min(PRICE_H, Math.max(11, t.y + 4))}>{t.label}</text>
      {/each}

      {#each data.gaps as g (g.from_ms)}
        <rect class="gap" x={sx(g.from_ms)} y="0" width={Math.max(2, sx(g.to_ms) - sx(g.from_ms))} height={H} />
      {/each}

      {#if hasQuotes}
        <path class="quote-band" d={area(data.ask, data.bid)} />
      {/if}

      {#if data.shape === "candles"}
        {#each data.x as x, i (x)}
          {#if data.high[i] !== null && data.low[i] !== null}
            {@const up = (data.close[i] ?? 0) >= (data.open[i] ?? 0)}
            <line class="wick" class:up class:down={!up} x1={sx(x)} x2={sx(x)} y1={sy(data.high[i]!)} y2={sy(data.low[i]!)} />
            {#if data.open[i] !== null && data.close[i] !== null}
              <rect
                class="body" class:up class:down={!up}
                x={sx(x) - bodyW / 2}
                y={Math.min(sy(data.open[i]!), sy(data.close[i]!))}
                width={bodyW}
                height={Math.max(1, Math.abs(sy(data.open[i]!) - sy(data.close[i]!)))}
              />
            {/if}
          {/if}
        {/each}
      {:else if data.shape === "line"}
        <path class="envelope" d={area(data.high, data.low)} />
        <path class="price" d={path_(data.close)} />
      {:else}
        <path class="price" d={path_(data.bid.map((b, i) => (b !== null && data!.ask[i] !== null ? (b + data!.ask[i]!) / 2 : null)))} />
      {/if}

      <g transform="translate(0,{PRICE_H + GAP_Y})">
        {#each data.x as x, i (x)}
          {#if data.volume[i]}
            <rect class="vol" x={sx(x) - bodyW / 2} y={VOL_H - sv(data.volume[i]!)} width={bodyW} height={sv(data.volume[i]!)} />
          {/if}
        {/each}
      </g>

      {#each xTicks as t (t.x)}
        <text class="axis" x={t.x} y={H + 14} text-anchor={t.x === 0 ? "start" : t.x >= PLOT_W - 1 ? "end" : "middle"}>{t.label}</text>
      {/each}

      {#if hover !== null}
        <line class="cross" x1={sx(data.x[hover])} x2={sx(data.x[hover])} y1="0" y2={H} />
      {/if}
    </svg>

    <div class="readout tabnum">
      {#if hover !== null}
        {@const i = hover}
        <span class="fg-muted">{fmtTime(data.x[i], data.daily)}{data.daily ? "" : ` · ${new Date(data.x[i]).toISOString().slice(0, 10)}`}</span>
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
          {fmtNum(data.rows)} rows{data.x.length < data.rows ? `, drawn as ${data.x.length} buckets` : ""} · hover for values
        </span>
      {/if}
    </div>

    {#if data.gaps.length}
      <ul class="gaps">
        {#each data.gaps as g (g.from_ms)}
          <li><AlertTriangle size={13} /> {fmtGap(g, data.daily)}</li>
        {/each}
      </ul>
    {:else}
      <p class="complete"><CheckCircle2 size={13} /> No gaps — rows arrive at a steady rhythm from start to end.</p>
    {/if}
  </div>
{/if}

<style>
  .chart { display: flex; flex-direction: column; gap: var(--sp-3); padding: var(--sp-4) var(--sp-5); }
  svg { width: 100%; height: 380px; display: block; touch-action: none; }
  .grid { stroke: var(--border); stroke-width: 1; vector-effect: non-scaling-stroke; }
  .axis { fill: var(--fg-subtle); font-size: 11px; font-family: var(--font-numeric); }
  .gap { fill: var(--bad); opacity: 0.12; }
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

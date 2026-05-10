<script lang="ts">
  /**
   * Calendar heatmap showing which trading days the user has on disk
   * (`coverage_report`) versus which exist upstream (`*_list_dates`).
   * Click any "missing" cell to bulk-queue the gap into the queue.
   *
   * Cell colours:
   *   accent      have it (local)
   *   surface-3   missing but available upstream
   *   subtle dot  not a trading day
   */
  import { onMount } from "svelte";
  import { Loader2, Plus, AlertTriangle } from "lucide-svelte";
  import { api, type Coverage, TAURI_AVAILABLE } from "$lib/api";
  import { app, log, openComposer, DATASETS } from "$lib/stores/app.svelte";
  // DATASETS is used as fallback for the composer anchor until catalogue-driven detail is wired

  let {
    symbol = "QQQ",
    kind = "stock_trade_quote",
  }: { symbol?: string; kind?: string } = $props();

  let upstream = $state<string[]>([]);
  let local = $state<string[]>([]);
  let loading = $state(false);
  let err = $state<string | null>(null);

  // Resolve which list-dates endpoint to use based on the kind.
  const isOption = $derived(kind.startsWith("option_"));
  const requestType = $derived(kind.includes("quote") ? "QUOTE" : "TRADE");

  async function load() {
    if (!TAURI_AVAILABLE) return;
    loading = true;
    err = null;
    try {
      const args: Record<string, string> = { request_type: requestType, symbol };
      const endpoint = isOption ? "option_list_dates" : "stock_list_dates";
      upstream = await api.listQuery({ endpoint, args });
      const cov: Coverage[] = await api.coverage();
      const found = cov.find((c) => c.symbol === symbol && c.kind === kind);
      local = found ? expandRange(found.first, found.last) : [];
    } catch (e: unknown) {
      err = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }
  onMount(load);
  $effect(() => { void symbol; void kind; load(); });

  function expandRange(first: string | null, last: string | null): string[] {
    // Coverage gives min/max; we don't get the full per-day list back yet.
    // Approximate: every weekday between first and last is "have it".
    // The on-disk truth is a finer scan (TODO: expose per-day file index
    // from tdds-core::coverage::scan).
    if (!first || !last) return [];
    const out: string[] = [];
    const f = new Date(first);
    const l = new Date(last);
    while (f <= l) {
      const dow = f.getDay();
      if (dow !== 0 && dow !== 6) {
        out.push(f.toISOString().slice(0, 10));
      }
      f.setDate(f.getDate() + 1);
    }
    return out;
  }

  // Group upstream dates by year-month for the calendar grid.
  const grid = $derived.by(() => {
    const set = new Set(upstream.map((d) => d.replace(/-/g, "")));
    const localSet = new Set(local.map((d) => d.replace(/-/g, "")));
    type Cell = { ymd: string; date: string; have: boolean; available: boolean };
    type MonthGroup = { year: number; month: number; cells: Cell[] };
    if (upstream.length === 0) return [] as MonthGroup[];
    const sorted = [...upstream]
      .map((d) => d.replace(/-/g, ""))
      .filter((d) => /^\d{8}$/.test(d))
      .sort();
    const start = new Date(`${sorted[0].slice(0, 4)}-${sorted[0].slice(4, 6)}-01`);
    const last  = new Date(`${sorted[sorted.length - 1].slice(0, 4)}-${sorted[sorted.length - 1].slice(4, 6)}-01`);
    const out: MonthGroup[] = [];
    while (start <= last) {
      const y = start.getFullYear(), m = start.getMonth();
      const cells: Cell[] = [];
      const days = new Date(y, m + 1, 0).getDate();
      for (let d = 1; d <= days; d++) {
        const ymd = `${y}${String(m + 1).padStart(2, "0")}${String(d).padStart(2, "0")}`;
        const date = `${y}-${String(m + 1).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
        cells.push({ ymd, date, available: set.has(ymd), have: localSet.has(ymd) });
      }
      out.push({ year: y, month: m, cells });
      start.setMonth(start.getMonth() + 1);
    }
    return out;
  });

  function bulkQueueMissing() {
    const ds = DATASETS.find((d) => d.id === kind);
    if (!ds) return;
    openComposer(ds);
    app.composer.symbol = symbol;
    // Composer will pick up the existing date pickers; user confirms.
    log("info", `Composer opened for missing-range bulk queue`, { symbol, kind });
  }

  const stats = $derived.by(() => {
    const haveSet = new Set(local.map((d) => d.replace(/-/g, "")));
    const upSet   = new Set(upstream.map((d) => d.replace(/-/g, "")));
    const missing = [...upSet].filter((d) => !haveSet.has(d));
    return { upstream: upSet.size, have: haveSet.size, missing: missing.length };
  });
</script>

<div class="coverage">
  <header class="head">
    <div>
      <span class="text-caption">Coverage</span>
      <h2 class="title">{symbol} · <code>{kind}</code></h2>
    </div>
    <div class="stat-row tabnum">
      <div class="stat"><span class="k">Upstream</span><span class="v">{stats.upstream}</span></div>
      <div class="stat"><span class="k">On disk</span><span class="v ok">{stats.have}</span></div>
      <div class="stat"><span class="k">Missing</span><span class="v warn">{stats.missing}</span></div>
      {#if stats.missing > 0}
        <button class="btn btn-primary" onclick={bulkQueueMissing}>
          <Plus size={14} /> Queue missing
        </button>
      {/if}
    </div>
  </header>

  {#if loading}
    <div class="state"><Loader2 class="spin" size={14} /> Loading coverage…</div>
  {:else if err}
    <div class="state error"><AlertTriangle size={14} /> {err}</div>
  {:else if upstream.length === 0}
    <div class="state">No upstream dates available — check connection / subscription tier.</div>
  {:else}
    <div class="months">
      {#each grid as month}
        <div class="month">
          <span class="month-label text-caption">
            {new Date(month.year, month.month, 1).toLocaleString(undefined, { month: "short", year: "2-digit" })}
          </span>
          <div class="cells">
            {#each month.cells as c}
              <span
                class="cell"
                class:available={c.available}
                class:have={c.have}
                title="{c.date}{c.have ? ' · ✓ have' : c.available ? ' · missing upstream' : ' · not a trading day'}"
              ></span>
            {/each}
          </div>
        </div>
      {/each}
    </div>
    <div class="legend text-caption">
      <span class="cell legend-cell"></span> not a trading day
      <span class="cell available legend-cell"></span> available upstream
      <span class="cell have legend-cell"></span> on disk
    </div>
  {/if}
</div>

<style>
  .coverage { display: flex; flex-direction: column; gap: var(--sp-3); }
  .head {
    display: flex; justify-content: space-between; align-items: flex-start;
    gap: var(--sp-3);
  }
  .title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    margin: 0;
  }
  .title code {
    font-family: var(--font-mono);
    font-size: 0.6em;
    color: var(--fg-muted);
    background: var(--surface-3);
    padding: 1px 6px;
    border-radius: var(--r-sm);
    vertical-align: middle;
  }

  .stat-row { display: flex; gap: var(--sp-3); align-items: center; }
  .stat { display: flex; flex-direction: column; align-items: flex-end; }
  .stat .k { font-size: 10px; color: var(--fg-muted); text-transform: uppercase; letter-spacing: 0.05em; }
  .stat .v { font-family: var(--font-mono); font-size: 16px; }
  .stat .v.ok { color: var(--good); }
  .stat .v.warn { color: var(--warn); }

  .state {
    display: flex; align-items: center; gap: 6px;
    padding: var(--sp-4); border: 1px dashed var(--border);
    border-radius: var(--r-md);
    background: var(--surface-1);
    color: var(--fg-muted);
  }
  .state.error { color: var(--bad); border-color: rgba(255,126,126,0.3); }

  .months {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: var(--sp-3);
  }
  .month {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: var(--sp-2) var(--sp-3);
  }
  .month-label { color: var(--fg-muted); display: block; margin-bottom: 4px; }
  .cells {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
  }
  .cell {
    width: 100%;
    aspect-ratio: 1;
    background: var(--surface-3);
    border-radius: 2px;
    border: 1px solid transparent;
  }
  .cell.available { background: rgba(124, 140, 255, 0.18); border-color: rgba(124, 140, 255, 0.3); }
  .cell.have      { background: var(--accent); border-color: var(--accent-hi); }
  .legend {
    display: inline-flex; align-items: center; gap: 6px;
    color: var(--fg-muted);
  }
  .legend-cell { width: 10px; aspect-ratio: 1; }
  :global(.spin) { animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>

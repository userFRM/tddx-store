<script lang="ts">
  /**
   * Bulk-queue dialog for an index ecosystem (S&P 500 / NDX / …).
   * Resolves the constituent list via indexkit, lets the user pick a
   * kind + date range + format, then enqueues one task per (constituent,
   * date) pair.
   */
  import { X, Loader2, Layers, ChevronRight } from "lucide-svelte";
  import { app, closeIndexPreset, log } from "$lib/stores/app.svelte";
  import { api, TAURI_AVAILABLE } from "$lib/api";

  let symbols = $state<string[]>([]);
  let loadingSymbols = $state(false);
  let kind = $state("stock_trade_quote");
  let start = $state("");
  let end = $state("");
  let format = $state<"parquet" | "csv" | "jsonl" | "json">("parquet");
  let topN = $state(0);     // 0 = all
  let busy = $state(false);
  let msg = $state("");

  // Resolve constituents whenever the modal opens with a different preset.
  let lastId = "";
  $effect(() => {
    if (!app.presetOpen || !app.presetSelected) return;
    if (app.presetSelected.id === lastId) return;
    lastId = app.presetSelected.id;
    symbols = [];
    msg = "";
    void resolve();
  });

  async function resolve() {
    if (!app.presetSelected) return;
    if (!TAURI_AVAILABLE) {
      msg = "Browser preview — connect via the desktop app to fetch constituents.";
      return;
    }
    loadingSymbols = true;
    try {
      const list = await api.indexConstituents(app.presetSelected.id);
      symbols = list;
      log("info", `Resolved ${list.length} constituents for ${app.presetSelected.name}`);
    } catch (e: unknown) {
      msg = e instanceof Error ? e.message : String(e);
      log("warn", `Constituent fetch failed: ${msg}`);
    } finally {
      loadingSymbols = false;
    }
  }

  async function bulkQueue() {
    if (!app.presetSelected) return;
    const eligible = topN > 0 ? symbols.slice(0, topN) : symbols;
    if (eligible.length === 0) { msg = "No symbols resolved."; return; }
    if (!start && !end) { msg = "Set a start date or a date range."; return; }
    busy = true;
    msg = `Queueing ${eligible.length} symbols…`;
    let total = 0;
    let firstErr = "";
    for (const symbol of eligible) {
      try {
        const n = await api.enqueue({
          kind,
          symbol,
          format,
          start: start || null,
          end: end || null,
          date: !end ? (start || null) : null,
          interval: "0",
          expiration: "*",
          strike: "*",
          right: "both",
        });
        total += n;
      } catch (e: unknown) {
        if (!firstErr) firstErr = e instanceof Error ? e.message : String(e);
      }
    }
    busy = false;
    if (firstErr && total === 0) {
      msg = firstErr;
      log("error", `Bulk-queue failed: ${firstErr}`);
    } else {
      msg = `Queued ${total} task${total === 1 ? "" : "s"} across ${eligible.length} symbol${eligible.length === 1 ? "" : "s"}${firstErr ? " (some failed)" : ""}`;
      log("info", `Bulk-queued ${app.presetSelected.id}`, { tasks: total, symbols: eligible.length });
      setTimeout(closeIndexPreset, 1200);
    }
  }
</script>

{#if app.presetOpen && app.presetSelected}
  <div class="backdrop" onclick={closeIndexPreset} role="presentation">
    <div class="card" onclick={(e) => e.stopPropagation()}
         role="dialog" aria-modal="true" tabindex="-1">
      <header class="head">
        <div>
          <span class="text-caption">Bulk queue · index ecosystem</span>
          <h2 class="title"><Layers size={18} /> {app.presetSelected.name}</h2>
          <p class="sub fg-muted">{app.presetSelected.description}</p>
        </div>
        <button class="btn-icon" onclick={closeIndexPreset} aria-label="Close">
          <X size={14} />
        </button>
      </header>

      <div class="resolved">
        {#if loadingSymbols}
          <Loader2 class="spin" size={14} />
          <span class="fg-muted">Fetching constituents…</span>
        {:else if symbols.length === 0}
          <span class="fg-muted">No constituents loaded.</span>
        {:else}
          <span class="tabnum">{symbols.length} constituents</span>
          <span class="dot">·</span>
          <span class="fg-muted">First: {symbols.slice(0, 6).join(", ")}…</span>
        {/if}
      </div>

      <div class="form">
        <label class="field">
          <span class="text-caption">Kind</span>
          <select class="field-input" bind:value={kind}>
            <option value="stock_trade">stock_trade — every trade</option>
            <option value="stock_quote">stock_quote — every NBBO update</option>
            <option value="stock_trade_quote">stock_trade_quote</option>
            <option value="option_trade">option_trade — every trade (full chain per symbol)</option>
            <option value="option_quote">option_quote — full chain</option>
            <option value="option_trade_quote">option_trade_quote — full chain</option>
            <option value="option_oi">option_oi — daily OI</option>
          </select>
        </label>

        <div class="row">
          <label class="field">
            <span class="text-caption">Start date</span>
            <input class="field-input" bind:value={start} placeholder="YYYY-MM-DD" />
          </label>
          <label class="field">
            <span class="text-caption">End date</span>
            <input class="field-input" bind:value={end} placeholder="YYYY-MM-DD" />
          </label>
        </div>

        <div class="row">
          <label class="field">
            <span class="text-caption">Format</span>
            <select class="field-input" bind:value={format}>
              <option value="parquet">Parquet (zstd)</option>
              <option value="csv">CSV</option>
              <option value="jsonl">JSON Lines</option>
              <option value="json">JSON array</option>
            </select>
          </label>
          <label class="field">
            <span class="text-caption">Top N (0 = all)</span>
            <input class="field-input tabnum" type="number" min="0" max={symbols.length}
                   bind:value={topN} />
          </label>
        </div>
      </div>

      <footer class="foot">
        <span class="msg">{msg}</span>
        <button class="btn btn-primary" onclick={bulkQueue} disabled={busy || symbols.length === 0}>
          {#if busy}
            <Loader2 class="spin" size={14} />Queueing…
          {:else}
            Queue {topN > 0 ? Math.min(topN, symbols.length) : symbols.length} symbols
            <ChevronRight size={14} />
          {/if}
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed; inset: 0;
    background: rgba(8,11,18,0.55);
    backdrop-filter: blur(4px);
    display: flex; align-items: center; justify-content: center;
    z-index: 90;
  }
  .card {
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-modal);
    width: 620px;
    max-width: calc(100vw - var(--sp-8));
    display: flex; flex-direction: column;
  }
  .head {
    display: flex; justify-content: space-between; gap: var(--sp-3);
    padding: var(--sp-5);
    border-bottom: 1px solid var(--border);
  }
  .title {
    display: inline-flex; align-items: center; gap: 10px;
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    margin: 0;
  }
  .sub { font-size: var(--text-body-sm); margin: 4px 0 0; }
  .resolved {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-5);
    border-bottom: 1px solid var(--border);
    color: var(--fg);
    font-family: var(--font-mono);
  }
  .dot { color: var(--fg-subtle); }
  .form {
    padding: var(--sp-4) var(--sp-5);
    display: flex; flex-direction: column; gap: var(--sp-3);
  }
  .row { display: grid; grid-template-columns: 1fr 1fr; gap: var(--sp-3); }
  .field { display: flex; flex-direction: column; gap: 4px; }
  .foot {
    display: flex; justify-content: space-between; align-items: center; gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-5);
    border-top: 1px solid var(--border);
    background: var(--surface-1);
    border-radius: 0 0 var(--r-lg) var(--r-lg);
  }
  .msg { color: var(--fg-muted); font-size: var(--text-body-sm); }
  :global(.spin) { animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>

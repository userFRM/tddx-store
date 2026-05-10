<script lang="ts">
  /**
   * Parquet preview modal. Backed by the `parquet_preview` Tauri
   * command which decodes N rows of any local parquet file into JSON.
   * Used by the Library "Sample" button and the Detail page Sample tab.
   *
   * Virtualised list isn't strictly necessary at the default 100-row
   * limit, but row width on dense schemas (TradeQuoteTick has 27 cols)
   * benefits from a horizontal scroll container with sticky header.
   */
  import { onMount } from "svelte";
  import { X, Loader2, Download, ChevronLeft, ChevronRight } from "lucide-svelte";
  import { api, fmtBytes, fmtNum, type PreviewResult } from "$lib/api";
  import { app } from "$lib/stores/app.svelte";

  let {
    open = $bindable(false),
    path = "",
    title = "",
  }: { open: boolean; path: string; title?: string } = $props();

  let result = $state<PreviewResult | null>(null);
  let loading = $state(false);
  let err = $state<string | null>(null);
  let offset = $state(0);
  const LIMIT = 100;

  async function load() {
    if (!open || !path) return;
    loading = true;
    err = null;
    try {
      result = await api.parquetPreview({ path, offset, limit: LIMIT });
    } catch (e: unknown) {
      err = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => { void open; void path; void offset; load(); });

  function close() { open = false; result = null; offset = 0; }

  function fmtCell(v: unknown): string {
    if (v === null || v === undefined) return "—";
    if (typeof v === "number") return v.toLocaleString();
    if (typeof v === "string" && v.length > 64) return v.slice(0, 64) + "…";
    return String(v);
  }

  function isNumeric(dtype: string): boolean {
    return /Int|Float|UInt/.test(dtype);
  }

  function nextPage() {
    if (!result) return;
    if (offset + LIMIT < result.total_rows) offset += LIMIT;
  }
  function prevPage() {
    offset = Math.max(0, offset - LIMIT);
  }
</script>

{#if open}
  <div class="backdrop" onclick={close} role="presentation">
    <div class="card" onclick={(e) => e.stopPropagation()}
         role="dialog" aria-modal="true" aria-label="Parquet preview"
         tabindex="-1"
         onkeydown={(e) => e.key === "Escape" && close()}>
      <header class="head">
        <div class="title-block">
          <span class="text-caption">Sample</span>
          <h2 class="title">{title || "Parquet preview"}</h2>
          <p class="path text-mono">{path}</p>
          {#if result}
            <div class="meta tabnum">
              <span>{fmtNum(result.total_rows)} rows total</span>
              <span class="sep">·</span>
              <span>{result.schema.length} columns</span>
              <span class="sep">·</span>
              <span>{fmtBytes(result.bytes)}</span>
            </div>
          {/if}
        </div>
        <button class="btn-icon" onclick={close} aria-label="Close"><X size={14} /></button>
      </header>

      <div class="body">
        {#if loading && !result}
          <div class="state"><Loader2 class="spin" size={16} /> Decoding parquet…</div>
        {:else if err}
          <div class="state error">{err}</div>
        {:else if result && result.rows.length === 0}
          <div class="state">Empty file.</div>
        {:else if result}
          <div class="table-wrap">
            <table class="grid">
              <thead><tr>
                {#each result.schema as col}
                  <th class={isNumeric(col.dtype) ? "num" : ""}>
                    <span class="col-name">{col.name}</span>
                    <span class="col-type text-caption">{col.dtype.replace(/^[A-Z]+\(/, "").replace(/\)$/, "")}</span>
                  </th>
                {/each}
              </tr></thead>
              <tbody>
                {#each result.rows as row}
                  <tr>
                    {#each row as cell, i}
                      <td class={isNumeric(result.schema[i].dtype) ? "num tabnum" : ""}>
                        {fmtCell(cell)}
                      </td>
                    {/each}
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>

      {#if result}
        <footer class="foot">
          <span class="hint fg-muted tabnum">
            Rows {fmtNum(offset + 1)}–{fmtNum(offset + result.returned)} of {fmtNum(result.total_rows)}
          </span>
          <div class="page-nav">
            <button class="btn btn-ghost" onclick={prevPage} disabled={offset === 0}>
              <ChevronLeft size={14} /> Prev
            </button>
            <button
              class="btn btn-ghost"
              onclick={nextPage}
              disabled={offset + LIMIT >= result.total_rows}
            >
              Next <ChevronRight size={14} />
            </button>
          </div>
        </footer>
      {/if}
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
    padding: var(--sp-4);
  }
  .card {
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-modal);
    width: 100%;
    max-width: 1200px;
    max-height: calc(100vh - var(--sp-12));
    display: flex; flex-direction: column;
    overflow: hidden;
  }
  .head {
    display: flex; justify-content: space-between; gap: var(--sp-3);
    padding: var(--sp-4) var(--sp-5);
    border-bottom: 1px solid var(--border);
  }
  .title-block { min-width: 0; }
  .title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    margin: 0;
  }
  .path {
    color: var(--fg-subtle);
    font-size: var(--text-caption);
    margin-top: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 720px;
  }
  .meta { display: inline-flex; gap: 6px; margin-top: 6px; color: var(--fg-muted); font-size: var(--text-body-sm); }
  .meta .sep { color: var(--fg-subtle); }

  .body {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .state {
    padding: var(--sp-8);
    text-align: center;
    color: var(--fg-muted);
    display: inline-flex; align-items: center; gap: var(--sp-2);
    justify-content: center;
  }
  .state.error { color: var(--bad); }

  .table-wrap {
    overflow: auto;
    flex: 1;
  }
  .grid {
    width: max-content;
    min-width: 100%;
    border-collapse: collapse;
  }
  .grid thead {
    position: sticky;
    top: 0;
    background: var(--surface-1);
    z-index: 1;
  }
  .grid th {
    text-align: left;
    padding: 6px var(--sp-3);
    font-weight: var(--weight-semi);
    border-bottom: 1px solid var(--border);
    font-size: var(--text-body-sm);
    white-space: nowrap;
  }
  .grid th.num { text-align: right; }
  .col-name { display: block; color: var(--fg); }
  .col-type { display: block; color: var(--fg-subtle); margin-top: 1px; }

  .grid td {
    padding: 4px var(--sp-3);
    border-bottom: 1px solid var(--border);
    font-size: var(--text-body-sm);
    white-space: nowrap;
    color: var(--fg);
  }
  .grid td.num { text-align: right; font-family: var(--font-mono); }
  .grid tbody tr:hover { background: var(--surface-1); }

  .foot {
    display: flex; justify-content: space-between; align-items: center;
    padding: var(--sp-2) var(--sp-5);
    border-top: 1px solid var(--border);
    background: var(--surface-1);
  }
  .hint { font-size: var(--text-body-sm); }
  .page-nav { display: inline-flex; gap: 6px; }
</style>

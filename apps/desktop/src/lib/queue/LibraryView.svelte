<script lang="ts">
  import {
    Search,
    ChevronDown,
    ChevronRight,
    FolderOpen,
    RotateCcw,
    Plus,
    ArrowRight,
    Library,
  } from "lucide-svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api, fmtBytes, fmtNum, type Coverage } from "$lib/api";
  import { app, navigate, log, composer } from "$lib/stores/app.svelte";
  import { onMount } from "svelte";

  type SortKey = "symbol" | "size" | "files" | "recent";

  const SORT_LABELS: Record<SortKey, string> = {
    symbol: "Symbol A-Z",
    size: "Largest first",
    files: "Most files",
    recent: "Most recent",
  };

  let coverage = $state<Coverage[]>([]);
  let loading = $state(true);
  let rowMsg = $state("");
  let sortKey = $state<SortKey>("symbol");
  let busy = $state(false);

  /** Queue every trading day missing from a set's own span. */
  async function refillGaps(row: Coverage) {
    rowMsg = `Checking ${row.symbol} ${row.kind}…`;
    try {
      const n = await api.requeueMissingDates(row.kind, row.symbol);
      rowMsg =
        n === 0
          ? `${row.symbol} ${row.kind} has no gaps`
          : `Queued ${n} missing day${n === 1 ? "" : "s"} for ${row.symbol}`;
      log("info", rowMsg);
    } catch (e: unknown) {
      rowMsg = e instanceof Error ? e.message : String(e);
      log("error", `Refill failed: ${rowMsg}`);
    }
    setTimeout(() => (rowMsg = ""), 3000);
  }

  /** Reveal the dataset's directory in the OS file manager. */
  async function revealKindDir(row: Coverage) {
    try {
      await revealItemInDir(`${app.settings.output_dir}/${row.kind}`);
    } catch (e: unknown) {
      rowMsg = e instanceof Error ? e.message : String(e);
      setTimeout(() => (rowMsg = ""), 3000);
    }
  }
  let filterQuery = $state("");
  let expandedSymbols = $state<Set<string>>(new Set());

  onMount(async () => {
    try {
      coverage = await api.coverage();
    } catch {
      // not connected
    } finally {
      loading = false;
    }
  });

  // Group by symbol, filtering on the symbol AND the dataset, because
  // "show me everything with greeks" is as common a question as
  // "show me everything for QQQ".
  const grouped = $derived.by<[string, Coverage[]][]>(() => {
    const q = filterQuery.trim().toLowerCase();
    const map = new Map<string, Coverage[]>();
    for (const row of coverage) {
      if (
        q &&
        !row.symbol.toLowerCase().includes(q) &&
        !row.kind.toLowerCase().includes(q) &&
        !kindLabel(row.kind).toLowerCase().includes(q)
      ) {
        continue;
      }
      if (!map.has(row.symbol)) map.set(row.symbol, []);
      map.get(row.symbol)!.push(row);
    }
    const groups = Array.from(map.entries());
    groups.sort(([aSym, aRows], [bSym, bRows]) => {
      switch (sortKey) {
        case "size":
          return symbolTotalBytes(bRows) - symbolTotalBytes(aRows);
        case "files":
          return symbolTotalFiles(bRows) - symbolTotalFiles(aRows);
        case "recent":
          return (lastDate(bRows) ?? "").localeCompare(lastDate(aRows) ?? "");
        default:
          return aSym.localeCompare(bSym);
      }
    });
    return groups;
  });

  const allExpanded = $derived(
    grouped.length > 0 && grouped.every(([sym]) => expandedSymbols.has(sym)),
  );

  function toggleAll() {
    expandedSymbols = allExpanded
      ? new Set()
      : new Set(grouped.map(([sym]) => sym));
  }

  function lastDate(rows: Coverage[]): string | null {
    return rows.map((r) => r.last).filter(Boolean).sort().pop() ?? null;
  }

  /** Trading days inside a set's own span that are not on disk. Weekends
   *  and holidays are not gaps, so this only counts weekdays and still
   *  overstates around market holidays — the exact figure needs the
   *  server's calendar, which is what the refill call fetches. */
  function approxGaps(row: Coverage): number {
    if (!row.first || !row.last) return 0;
    const have = new Set(row.dates);
    let weekdays = 0;
    const cursor = new Date(row.first);
    const end = new Date(row.last);
    while (cursor <= end) {
      const dow = cursor.getUTCDay();
      if (dow !== 0 && dow !== 6 && !have.has(cursor.toISOString().slice(0, 10))) {
        weekdays += 1;
      }
      cursor.setUTCDate(cursor.getUTCDate() + 1);
    }
    return weekdays;
  }

  /** Fill gaps across every dataset held for one symbol. */
  async function refillSymbol(symbol: string, rows: Coverage[]) {
    if (busy) return;
    busy = true;
    rowMsg = `Checking ${symbol}…`;
    try {
      let queued = 0;
      for (const row of rows) {
        queued += await api.requeueMissingDates(row.kind, row.symbol);
      }
      rowMsg =
        queued === 0
          ? `${symbol} has no gaps`
          : `Queued ${queued} missing day${queued === 1 ? "" : "s"} for ${symbol}`;
      log("info", rowMsg);
    } catch (e: unknown) {
      rowMsg = e instanceof Error ? e.message : String(e);
      log("error", `Refill failed: ${rowMsg}`);
    } finally {
      busy = false;
      setTimeout(() => (rowMsg = ""), 3000);
    }
  }

  function toggleSymbol(symbol: string) {
    const s = new Set(expandedSymbols);
    if (s.has(symbol)) s.delete(symbol);
    else s.add(symbol);
    expandedSymbols = s;
  }

  function symbolTotalBytes(rows: Coverage[]): number {
    return rows.reduce((sum, r) => sum + r.bytes, 0);
  }
  function symbolTotalFiles(rows: Coverage[]): number {
    return rows.reduce((sum, r) => sum + r.files, 0);
  }
  function symbolSpan(rows: Coverage[]): string {
    const firsts = rows.map((r) => r.first).filter(Boolean) as string[];
    const lasts  = rows.map((r) => r.last).filter(Boolean) as string[];
    if (firsts.length === 0) return "—";
    const first = firsts.sort()[0];
    const last  = lasts.sort().reverse()[0];
    return `${first} → ${last}`;
  }

  // Kind label from catalogue or coverage kind string
  function kindLabel(kind: string): string {
    const entry = app.catalogue.find((e) => e.name === kind);
    if (entry) return entry.summary || kind;
    // Fallback: humanise snake_case
    return kind.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
  }

  // ── Never-downloaded section ──────────────────────────────────
  // Show catalogue entries that have no coverage rows
  const downloadedKinds = $derived(new Set(coverage.map((r) => r.kind)));

  const neverDownloaded = $derived.by(() => {
    if (app.catalogue.length === 0) return [];
    return app.catalogue.filter((e) => !downloadedKinds.has(e.name));
  });

  // Group never-downloaded by category
  const neverByCategory = $derived.by(() => {
    const order: string[] = [];
    const map = new Map<string, typeof app.catalogue>();
    for (const e of neverDownloaded) {
      const cat = e.category.charAt(0).toUpperCase() + e.category.slice(1);
      if (!map.has(cat)) { map.set(cat, []); order.push(cat); }
      map.get(cat)!.push(e);
    }
    return order.map((c) => ({ category: c, entries: map.get(c)! }));
  });

  let neverOpen = $state(false);

  /** Open Browse with the composer already pointed at this dataset and
   *  symbol, rather than dropping the user on an empty form. */
  function browseTo(kind: string, symbol = "") {
    composer.symbol = symbol;
    navigate("browse");
  }

  // First sentence of description
  function firstSentence(desc: string): string {
    if (!desc) return "";
    const m = desc.match(/^(.+?[.!?])\s/s);
    return m ? m[1] : desc.slice(0, 100);
  }
</script>

<div class="library-view">
  <!-- Header -->
  <div class="lib-header">
    <div class="header-left">
      <h1 class="lib-title">Library</h1>
      {#if coverage.length > 0}
        <span class="lib-meta text-mono fg-muted">
          {grouped.length} symbols · {fmtBytes(coverage.reduce((s, r) => s + r.bytes, 0))} total
        </span>
      {/if}
    </div>

    {#if rowMsg}
      <span class="row-msg text-body-sm" role="status">{rowMsg}</span>
    {/if}

    <div class="header-controls">
      {#if grouped.length > 0}
        <button class="btn btn-ghost" onclick={toggleAll}>
          {allExpanded ? "Collapse all" : "Expand all"}
        </button>
      {/if}

      <label class="sort-control">
        <span class="sr-only">Sort by</span>
        <select class="sort-select" bind:value={sortKey} aria-label="Sort by">
          {#each Object.entries(SORT_LABELS) as [key, label]}
            <option value={key}>{label}</option>
          {/each}
        </select>
      </label>

      <div class="search-wrap">
        <Search size={14} strokeWidth={1.75} class="search-icon" aria-hidden="true" />
        <input
          class="search-input"
          type="search"
          placeholder="Filter symbol or dataset…"
          bind:value={filterQuery}
          aria-label="Filter by symbol or dataset"
        />
      </div>
    </div>
  </div>

  <!-- Content -->
  <div class="lib-body">
    {#if loading}
      <div class="loading-state">
        <div class="spinner" aria-label="Loading library"></div>
        <span class="text-body-sm fg-muted">Loading library…</span>
      </div>
    {:else if grouped.length === 0 && !filterQuery}
      <div class="empty-state">
        <div class="empty-icon" aria-hidden="true">
          <Library size={40} strokeWidth={1.25} />
        </div>
        <p class="empty-label">Library is empty</p>
        <p class="text-body-sm fg-muted">Download datasets from Browse to see them here.</p>
        <button
          type="button"
          class="btn btn-primary empty-cta"
          onclick={() => navigate("browse")}
        >
          Browse datasets
          <ArrowRight size={14} strokeWidth={1.75} />
        </button>
      </div>
    {:else}
      <!-- Downloaded symbol list -->
      {#if grouped.length > 0}
        <div class="symbol-list" role="list">
          {#each grouped as [symbol, rows] (symbol)}
            {@const expanded = expandedSymbols.has(symbol)}
            {@const symbolGaps = rows.reduce((n, r) => n + approxGaps(r), 0)}
            <div class="symbol-group" role="listitem">
              <button
                class="symbol-row"
                onclick={() => toggleSymbol(symbol)}
                aria-expanded={expanded}
                aria-label="Toggle {symbol}"
              >
                <div class="symbol-chevron">
                  {#if expanded}
                    <ChevronDown size={14} strokeWidth={1.75} />
                  {:else}
                    <ChevronRight size={14} strokeWidth={1.75} />
                  {/if}
                </div>
                <span class="symbol-ticker text-mono">{symbol}</span>
                <div class="symbol-summary">
                  <span class="sum-stat text-mono">{fmtNum(rows.length)} {rows.length === 1 ? "dataset" : "datasets"}</span>
                  <span class="sum-sep">·</span>
                  <span class="sum-stat text-mono">{fmtNum(symbolTotalFiles(rows))} files</span>
                  <span class="sum-sep">·</span>
                  <span class="sum-stat text-mono">{fmtBytes(symbolTotalBytes(rows))}</span>
                  <span class="sum-sep">·</span>
                  <span class="sum-stat text-mono">{symbolSpan(rows)}</span>
                </div>
              </button>

              <div class="symbol-aside">
                {#if symbolGaps > 0}
                  <span class="gap-pill" title="Weekdays inside the span with no file">
                    {fmtNum(symbolGaps)} missing
                  </span>
                {/if}
                <button
                  class="btn-icon"
                  onclick={() => refillSymbol(symbol, rows)}
                  disabled={busy}
                  title="Queue the missing dates across every dataset for {symbol}"
                  aria-label="Queue the missing dates for {symbol}"
                >
                  <RotateCcw size={13} strokeWidth={1.75} />
                </button>
              </div>

              {#if expanded}
                <div class="kind-rows">
                  {#each rows as row (row.kind)}
                    <div class="kind-row">
                      <div class="kind-info">
                        <code class="kind-name">{row.kind}</code>
                        <span class="kind-title text-body-sm fg-muted">{kindLabel(row.kind)}</span>
                      </div>
                      <div class="kind-stats text-mono">
                        <span>{fmtNum(row.files)} files</span>
                        <span class="sum-sep">·</span>
                        <span>{fmtBytes(row.bytes)}</span>
                        <span class="sum-sep">·</span>
                        <span>{row.format}</span>
                        <span class="sum-sep">·</span>
                        <span>{row.first ?? "—"} → {row.last ?? "—"}</span>
                        {#if approxGaps(row) > 0}
                          <span class="sum-sep">·</span>
                          <span class="gap-count">{fmtNum(approxGaps(row))} missing</span>
                        {/if}
                      </div>
                      <div class="kind-actions">
                        <button
                          class="btn-icon"
                          onclick={() => browseTo(row.kind, symbol)}
                          title="Download more dates"
                          aria-label="Download more dates for {symbol} {row.kind}"
                        >
                          <Plus size={13} strokeWidth={1.75} />
                        </button>
                        <button
                          class="btn-icon"
                          onclick={() => refillGaps(row)}
                          title="Queue the missing dates in this range"
                          aria-label="Queue the missing dates for {symbol} {row.kind}"
                        >
                          <RotateCcw size={13} strokeWidth={1.75} />
                        </button>
                        <button
                          class="btn-icon"
                          onclick={() => revealKindDir(row)}
                          title="Show the output directory"
                          aria-label="Show the output directory for {symbol} {row.kind}"
                        >
                          <FolderOpen size={13} strokeWidth={1.75} />
                        </button>
                      </div>
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {:else if filterQuery}
        <div class="empty-state">
          <p class="empty-label">No results</p>
          <p class="text-body-sm fg-muted">No symbols matching "{filterQuery}".</p>
        </div>
      {/if}

      <!-- ── Available datasets not yet on disk ─────────────────── -->
      {#if !filterQuery && neverDownloaded.length > 0}
        <div class="never-section">
          <button
            type="button"
            class="never-toggle"
            onclick={() => (neverOpen = !neverOpen)}
            aria-expanded={neverOpen}
          >
            {#if neverOpen}
              <ChevronDown size={14} strokeWidth={1.75} />
            {:else}
              <ChevronRight size={14} strokeWidth={1.75} />
            {/if}
            <span class="never-toggle-label">
              Available datasets you don't have on disk
            </span>
            <span class="never-count text-caption tabnum">{neverDownloaded.length}</span>
          </button>

          {#if neverOpen}
            <div class="never-body">
              {#each neverByCategory as group (group.category)}
                <div class="never-category">
                  <div class="never-cat-label text-caption">{group.category}</div>
                  <div class="never-grid">
                    {#each group.entries as entry (entry.name)}
                      <div class="never-card">
                        <div class="never-card-top">
                          <span class="never-card-name">{entry.summary || entry.name}</span>
                          {#if entry.min_tier}
                            <span class="tier-pill tier-{entry.min_tier.toLowerCase()}">{entry.min_tier}</span>
                          {/if}
                        </div>
                        <p class="never-card-desc">{firstSentence(entry.description)}</p>
                        <button
                          type="button"
                          class="never-browse-btn"
                          onclick={() => browseTo(entry.name)}
                          aria-label="Browse {entry.summary}"
                        >
                          Browse
                          <ArrowRight size={12} strokeWidth={1.75} />
                        </button>
                      </div>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .library-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  /* Header */
  .row-msg {
    color: var(--fg-muted);
    margin-left: auto;
    padding-right: var(--sp-3);
    white-space: nowrap;
  }

  .lib-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
    padding: var(--sp-6) var(--sp-8) var(--sp-4);
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
  }

  .header-left {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .lib-title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    letter-spacing: -0.015em;
    color: var(--fg);
    line-height: 1.15;
  }

  .lib-meta {
    font-size: var(--text-body-sm);
    font-variant-numeric: tabular-nums;
  }

  /* Search */
  .search-wrap {
    position: relative;
    display: flex;
    align-items: center;
  }

  :global(.search-wrap .search-icon) {
    position: absolute;
    left: var(--sp-3);
    color: var(--fg-subtle);
    pointer-events: none;
  }

  .search-input {
    height: 32px;
    padding: 0 var(--sp-3) 0 calc(var(--sp-3) + 14px + var(--sp-2));
    width: 220px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--fg);
    font-size: var(--text-body-sm);
    font-family: var(--font-ui);
    outline: none;
    transition:
      border-color var(--dur-fast) var(--ease-standard),
      box-shadow var(--dur-fast) var(--ease-standard);
  }
  .search-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-tint);
  }
  .search-input::placeholder { color: var(--fg-subtle); }
  .search-input::-webkit-search-cancel-button { display: none; }

  /* Body */
  .lib-body {
    flex: 1;
    overflow-y: auto;
    padding: var(--sp-2) 0;
  }

  .symbol-list { display: flex; flex-direction: column; }

  /* Symbol group */
  .symbol-group {
    border-bottom: 1px solid var(--border);
  }
  .symbol-group:last-child { border-bottom: none; }

  .symbol-row {
    display: grid;
    grid-template-columns: 20px 80px 1fr;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-8);
    background: none;
    border: none;
    cursor: pointer;
    width: 100%;
    text-align: left;
    transition: background var(--dur-fast) var(--ease-standard);
    outline: none;
  }
  .symbol-row:hover { background: var(--surface-1); }
  .symbol-row:focus-visible { box-shadow: inset var(--shadow-glow-accent); }

  .symbol-chevron {
    color: var(--fg-subtle);
    display: flex;
    align-items: center;
  }

  .symbol-ticker {
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    color: var(--fg);
    font-variant-numeric: tabular-nums;
  }

  .symbol-summary {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }

  .sum-stat {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
  }

  .sum-sep { color: var(--fg-subtle); font-size: var(--text-body-sm); }

  /* Kind rows */
  .kind-rows {
    background: var(--surface-1);
    border-top: 1px solid var(--border);
  }

  .kind-row {
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-3) var(--sp-8) var(--sp-3) calc(var(--sp-8) + 80px + var(--sp-3));
    border-bottom: 1px solid var(--border);
    transition: background var(--dur-fast) var(--ease-standard);
  }
  .kind-row:last-child { border-bottom: none; }
  .kind-row:hover { background: var(--surface-2); }

  .kind-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .kind-name {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    color: var(--fg-muted);
  }

  .kind-title {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .kind-stats {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .kind-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-standard);
  }
  .kind-row:hover .kind-actions { opacity: 1; }

  /* States */
  .loading-state {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-8);
  }

  .spinner {
    width: 16px; height: 16px;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-16);
    text-align: center;
  }
  .empty-icon { opacity: 0.4; }
  .empty-label {
    font-size: var(--text-heading);
    font-weight: var(--weight-semi);
    color: var(--fg-muted);
  }

  .empty-cta {
    height: 36px;
    padding: 0 var(--sp-5);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    border-radius: var(--r-md);
    gap: var(--sp-2);
    margin-top: var(--sp-2);
  }

  /* ── Never-downloaded section ──────────────────────────────── */
  .never-section {
    border-top: 1px solid var(--border);
    padding: var(--sp-4) 0;
  }

  .never-toggle {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) var(--sp-8);
    background: none;
    border: none;
    cursor: pointer;
    width: 100%;
    text-align: left;
    color: var(--fg-muted);
    outline: none;
    transition: color var(--dur-fast) var(--ease-standard);
  }

  .never-toggle:hover { color: var(--fg); }
  .never-toggle:focus-visible { box-shadow: inset var(--shadow-glow-accent); }

  .never-toggle-label {
    flex: 1;
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    color: inherit;
  }

  .never-count {
    color: var(--fg-subtle);
    font-weight: var(--weight-normal);
    text-transform: none;
    letter-spacing: 0;
  }

  .never-body {
    padding: var(--sp-4) var(--sp-8);
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
  }

  .never-category {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .never-cat-label {
    color: var(--fg-subtle);
    padding-bottom: var(--sp-1);
    border-bottom: 1px solid var(--border);
  }

  .never-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: var(--sp-3);
  }

  .never-card {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-4);
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    transition: background var(--dur-fast) var(--ease-standard);
  }

  .never-card:hover {
    background: var(--surface-2);
  }

  .never-card-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-2);
  }

  .never-card-name {
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    color: var(--fg);
    letter-spacing: -0.005em;
  }

  .never-card-desc {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    line-height: 1.4;
    flex: 1;
    margin: 0;
  }

  .never-browse-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    padding: 3px var(--sp-3);
    background: var(--accent-tint);
    border: 1px solid var(--accent-tint-strong);
    border-radius: var(--r-sm);
    color: var(--accent-hi);
    font-size: var(--text-caption);
    font-weight: var(--weight-semi);
    cursor: pointer;
    align-self: flex-end;
    transition:
      background var(--dur-fast) var(--ease-standard),
      filter var(--dur-fast) var(--ease-standard);
  }

  .never-browse-btn:hover {
    filter: brightness(1.1);
  }

  /* Tier pills in never-downloaded section */
  .tier-pill {
    display: inline-flex;
    padding: 2px 6px;
    border-radius: var(--r-pill);
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    white-space: nowrap;
    border: 1px solid transparent;
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .tier-unknown  { background: var(--surface-2);                     color: var(--fg-subtle);       }
</style>

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
  import { api, fmtBytes, fmtNum, type Coverage } from "$lib/api";
  import { app, navigate } from "$lib/stores/app.svelte";
  import { onMount } from "svelte";

  let coverage = $state<Coverage[]>([]);
  let loading = $state(true);
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

  // Group coverage by symbol
  const grouped = $derived(() => {
    const q = filterQuery.trim().toUpperCase();
    const map = new Map<string, Coverage[]>();
    for (const row of coverage) {
      if (q && !row.symbol.toUpperCase().includes(q)) continue;
      if (!map.has(row.symbol)) map.set(row.symbol, []);
      map.get(row.symbol)!.push(row);
    }
    return Array.from(map.entries()).sort((a, b) => a[0].localeCompare(b[0]));
  });

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

  const neverDownloaded = $derived(() => {
    if (app.catalogue.length === 0) return [];
    return app.catalogue.filter((e) => !downloadedKinds.has(e.name));
  });

  // Group never-downloaded by category
  const neverByCategory = $derived(() => {
    const order: string[] = [];
    const map = new Map<string, typeof app.catalogue>();
    for (const e of neverDownloaded()) {
      const cat = e.category.charAt(0).toUpperCase() + e.category.slice(1);
      if (!map.has(cat)) { map.set(cat, []); order.push(cat); }
      map.get(cat)!.push(e);
    }
    return order.map((c) => ({ category: c, entries: map.get(c)! }));
  });

  let neverOpen = $state(false);

  function browseTo(operationId: string) {
    // Navigate to browse — the user will select the kind manually for now
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
          {grouped().length} symbols · {fmtBytes(coverage.reduce((s, r) => s + r.bytes, 0))} total
        </span>
      {/if}
    </div>

    <div class="search-wrap">
      <Search size={14} strokeWidth={1.75} class="search-icon" aria-hidden="true" />
      <input
        class="search-input"
        type="search"
        placeholder="Filter symbol…"
        bind:value={filterQuery}
        aria-label="Filter by symbol"
      />
    </div>
  </div>

  <!-- Content -->
  <div class="lib-body">
    {#if loading}
      <div class="loading-state">
        <div class="spinner" aria-label="Loading library"></div>
        <span class="text-body-sm fg-muted">Loading library…</span>
      </div>
    {:else if grouped().length === 0 && !filterQuery}
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
      {#if grouped().length > 0}
        <div class="symbol-list" role="list">
          {#each grouped() as [symbol, rows] (symbol)}
            {@const expanded = expandedSymbols.has(symbol)}
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
                  <span class="sum-stat text-mono">{fmtNum(symbolTotalFiles(rows))} files</span>
                  <span class="sum-sep">·</span>
                  <span class="sum-stat text-mono">{fmtBytes(symbolTotalBytes(rows))}</span>
                  <span class="sum-sep">·</span>
                  <span class="sum-stat text-mono">{symbolSpan(rows)}</span>
                </div>
              </button>

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
                        <span>{row.first ?? "—"} → {row.last ?? "—"}</span>
                      </div>
                      <div class="kind-actions">
                        <button
                          class="btn-icon"
                          onclick={() => navigate("browse")}
                          title="Download more dates"
                          aria-label="Download more dates for {symbol} {row.kind}"
                        >
                          <Plus size={13} strokeWidth={1.75} />
                        </button>
                        <button
                          class="btn-icon"
                          title="Re-run missing dates"
                          aria-label="Re-run missing dates for {symbol} {row.kind}"
                        >
                          <RotateCcw size={13} strokeWidth={1.75} />
                        </button>
                        <button
                          class="btn-icon"
                          title="Open output directory"
                          aria-label="Open output directory for {symbol} {row.kind}"
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
      {#if !filterQuery && neverDownloaded().length > 0}
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
            <span class="never-count text-caption tabnum">{neverDownloaded().length}</span>
          </button>

          {#if neverOpen}
            <div class="never-body">
              {#each neverByCategory() as group (group.category)}
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
    border: 1px solid rgba(124, 140, 255, 0.25);
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
  .tier-free     { background: rgba(92, 101, 119, 0.15);             color: var(--fg-muted);        }
  .tier-value    { background: rgba(56, 132, 255, 0.10);             color: rgb(56, 132, 255);      border-color: rgba(56, 132, 255, 0.20);  }
  .tier-standard { background: rgba(34, 175, 109, 0.12);             color: rgb(34, 175, 109);      border-color: rgba(34, 175, 109, 0.22);  }
  .tier-pro      { background: rgba(244, 196, 48, 0.14);             color: rgb(212, 158, 0);       border-color: rgba(244, 196, 48, 0.30);  }
</style>

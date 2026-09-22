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
    Diff,
    Database,
  } from "lucide-svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { api, fmtBytes, fmtNum, type Coverage } from "$lib/api";
  import { app, navigate, log, refreshQueueSnapshot, browseTo, loadCoverage } from "$lib/stores/app.svelte";
  import { onMount } from "svelte";
  import CoverageDiff from "$lib/queue/CoverageDiff.svelte";

  type SortKey = "symbol" | "size" | "files" | "recent";

  const SORT_LABELS: Record<SortKey, string> = {
    symbol: "Symbol A-Z",
    size: "Largest first",
    files: "Most files",
    recent: "Most recent",
  };

  const coverage = $derived(app.coverage);
  const loading = $derived(app.coverageLoading && app.coverage.length === 0);
  let rowMsg = $state("");
  let sortKey = $state<SortKey>("symbol");
  let busy = $state(false);

  /** Queue every trading day missing from a set's own span. */
  async function refillGaps(row: Coverage) {
    // The count is now queued work, so the panel's copy of it is stale.
    const key = gapKey(row);
    const { [key]: _dropped, ...rest } = gaps;
    gaps = rest;
    openGaps = new Set([...openGaps].filter((k) => k !== key));
    rowMsg = `Checking ${row.symbol} ${row.kind}…`;
    try {
      const n = await api.requeueMissingDates(row.kind, row.symbol);
      await refreshQueueSnapshot();
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

  /** Copy a DuckDB bootstrap script that exposes every dataset on disk
   *  as a queryable view. A downloader whose output cannot be opened is
   *  half a tool; this is the shortest path from "downloaded" to
   *  "queried" without the app growing a SQL console. */
  async function copyDuckDbScript() {
    busy = true;
    try {
      const { sql } = await api.duckdbCommand(app.settings.output_dir);
      await writeText(sql);
      rowMsg = "DuckDB script copied — paste it into a duckdb session";
      log("info", rowMsg);
    } catch (e: unknown) {
      rowMsg = e instanceof Error ? e.message : String(e);
      log("error", `DuckDB export failed: ${rowMsg}`);
    } finally {
      busy = false;
      setTimeout(() => (rowMsg = ""), 4000);
    }
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
  /** "What landed since I last looked" — a snapshot diff over the
   *  same coverage rows, opened on demand so it costs nothing when
   *  closed. */
  let showDiff = $state(false);
  let filterQuery = $state("");
  let expandedSymbols = $state<Set<string>>(new Set());

  // Shared with Home, and invalidated by the queue poll when a task
  // finishes, so the numbers here do not go stale behind a download.
  onMount(() => void loadCoverage());

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

  /** Per-row gap state, keyed `kind|symbol`. Absent until the user
   *  asks: the answer needs the vendor's trading calendar, so it is a
   *  network call, not something to fire for every visible row. */
  type GapState =
    | { status: "loading" }
    | { status: "error"; message: string }
    | { status: "ready"; dates: string[] };
  let gaps = $state<Record<string, GapState>>({});
  let openGaps = $state<Set<string>>(new Set());

  const gapKey = (row: Coverage) => `${row.kind}|${row.symbol}`;

  /** Collapse a date list into contiguous runs, so 33 scattered days
   *  read as a handful of ranges instead of a wall of dates. */
  function toRanges(dates: string[]): { from: string; to: string; days: number }[] {
    const out: { from: string; to: string; days: number }[] = [];
    for (const date of dates) {
      const prev = out[out.length - 1];
      const dayAfter = prev
        ? new Date(new Date(prev.to + "T00:00:00Z").getTime() + 86_400_000)
            .toISOString()
            .slice(0, 10)
        : null;
      // Runs join across a weekend, since Saturday and Sunday are not
      // gaps and would otherwise split every week into its own range.
      const within = prev && date > prev.to && date <= addDays(prev.to, 3);
      if (prev && (date === dayAfter || within)) {
        prev.to = date;
        prev.days += 1;
      } else {
        out.push({ from: date, to: date, days: 1 });
      }
    }
    return out;
  }

  function addDays(iso: string, n: number): string {
    return new Date(new Date(iso + "T00:00:00Z").getTime() + n * 86_400_000)
      .toISOString()
      .slice(0, 10);
  }

  async function toggleGaps(row: Coverage) {
    const key = gapKey(row);
    const next = new Set(openGaps);
    if (next.has(key)) {
      next.delete(key);
      openGaps = next;
      return;
    }
    next.add(key);
    openGaps = next;
    if (gaps[key]?.status === "ready") return;
    gaps = { ...gaps, [key]: { status: "loading" } };
    try {
      const dates = await api.missingDates(row.kind, row.symbol);
      gaps = { ...gaps, [key]: { status: "ready", dates } };
    } catch (e: unknown) {
      gaps = {
        ...gaps,
        [key]: { status: "error", message: e instanceof Error ? e.message : String(e) },
      };
    }
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
        <span class="lib-meta text-figures fg-muted">
          {grouped.length} {grouped.length === 1 ? "symbol" : "symbols"} · {fmtBytes(coverage.reduce((s, r) => s + r.bytes, 0))} total
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

      {#if coverage.length > 0}
        <button
          class="btn btn-ghost"
          onclick={copyDuckDbScript}
          disabled={busy}
          title="Copy a DuckDB script that views every dataset on disk"
        >
          <Database size={14} strokeWidth={1.75} aria-hidden="true" />
          DuckDB
        </button>
      {/if}

      <button
        class="btn btn-ghost"
        class:active={showDiff}
        onclick={() => (showDiff = !showDiff)}
        aria-pressed={showDiff}
        title="Compare the library against a saved snapshot"
      >
        <Diff size={14} strokeWidth={1.75} aria-hidden="true" />
        Changes
      </button>

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

  {#if showDiff}
    <div class="diff-panel"><CoverageDiff /></div>
  {/if}

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
                <span class="symbol-ticker text-figures">{symbol}</span>
                <div class="symbol-summary">
                  <span class="sum-stat text-figures">{fmtNum(rows.length)} {rows.length === 1 ? "dataset" : "datasets"}</span>
                  <span class="sum-sep">·</span>
                  <span class="sum-stat text-figures">{fmtNum(symbolTotalFiles(rows))} files</span>
                  <span class="sum-sep">·</span>
                  <span class="sum-stat text-figures">{fmtBytes(symbolTotalBytes(rows))}</span>
                  <span class="sum-sep">·</span>
                  <span class="sum-stat text-figures">{symbolSpan(rows)}</span>
                </div>
              </button>

              <div class="symbol-aside">
                <button
                  class="btn btn-secondary btn-sm"
                  onclick={() => refillSymbol(symbol, rows)}
                  disabled={busy}
                  title="Check every dataset for {symbol} and queue whatever is missing"
                >
                  <RotateCcw size={12} strokeWidth={1.75} />
                  Fill gaps
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
                      <div class="kind-stats text-figures">
                        <span>{fmtNum(row.files)} files</span>
                        <span class="sum-sep">·</span>
                        <span>{fmtBytes(row.bytes)}</span>
                        <span class="sum-sep">·</span>
                        <span>{row.format}</span>
                        <span class="sum-sep">·</span>
                        <span>{row.first ?? "—"} → {row.last ?? "—"}</span>
                      </div>
                      <button
                        class="gap-toggle"
                        class:open={openGaps.has(gapKey(row))}
                        onclick={() => toggleGaps(row)}
                        aria-expanded={openGaps.has(gapKey(row))}
                      >
                        {#if gaps[gapKey(row)]?.status === "loading"}
                          Checking…
                        {:else if gaps[gapKey(row)]?.status === "ready"}
                          {@const n = (gaps[gapKey(row)] as { dates: string[] }).dates.length}
                          {n === 0 ? "Complete" : `${fmtNum(n)} missing`}
                        {:else}
                          Check gaps
                        {/if}
                      </button>

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

                    {#if openGaps.has(gapKey(row))}
                      {@const state = gaps[gapKey(row)]}
                      <div class="gap-panel">
                        {#if !state || state.status === "loading"}
                          <span class="gap-note fg-muted text-body-sm">
                            Asking ThetaData which days it has for {row.symbol}…
                          </span>
                        {:else if state.status === "error"}
                          <span class="gap-note text-body-sm" style="color: var(--bad)">
                            {state.message}
                          </span>
                        {:else if state.dates.length === 0}
                          <span class="gap-note fg-muted text-body-sm">
                            Every trading day between {row.first} and {row.last} is on disk.
                          </span>
                        {:else}
                          <div class="gap-head">
                            <span class="gap-note text-body-sm">
                              {fmtNum(state.dates.length)} trading
                              {state.dates.length === 1 ? "day" : "days"} missing between
                              {row.first} and {row.last}. Market holidays are excluded —
                              these are days ThetaData has and you do not.
                            </span>
                            <button
                              class="btn btn-primary btn-sm"
                              disabled={busy}
                              onclick={() => refillGaps(row)}
                            >
                              Queue {fmtNum(state.dates.length)}
                              {state.dates.length === 1 ? "day" : "days"}
                            </button>
                          </div>
                          <ul class="gap-ranges">
                            {#each toRanges(state.dates) as range}
                              <li class="gap-range text-figures">
                                {#if range.days === 1}
                                  {range.from}
                                {:else}
                                  {range.from} → {range.to}
                                  <span class="fg-subtle">({range.days})</span>
                                {/if}
                              </li>
                            {/each}
                          </ul>
                        {/if}
                      </div>
                    {/if}
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
  .gap-toggle {
    justify-self: end;
    padding: 2px var(--sp-2);
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    background: transparent;
    color: var(--fg-muted);
    font-size: var(--text-caption);
    font-family: var(--font-ui);
    cursor: pointer;
    white-space: nowrap;
    transition: border-color var(--dur-fast) var(--ease-standard),
                color var(--dur-fast) var(--ease-standard);
  }
  .gap-toggle:hover, .gap-toggle.open {
    border-color: var(--accent);
    color: var(--accent);
  }

  .gap-panel {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4);
    margin: 0 0 var(--sp-2) var(--sp-8);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
  }
  .gap-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-4);
  }
  .gap-note { max-width: 62ch; }
  .gap-ranges {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1) var(--sp-3);
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .gap-range {
    font-size: var(--text-caption);
    color: var(--fg-muted);
  }

  .row-msg {
    color: var(--fg-muted);
    margin-left: auto;
    padding-right: var(--sp-3);
    white-space: nowrap;
  }

  .diff-panel {
    border-bottom: 1px solid var(--border);
    padding: 0 var(--space-6) var(--space-4);
  }
  .btn-ghost.active {
    color: var(--accent);
    background: var(--surface-2);
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
    grid-template-columns: 1fr auto auto auto;
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
    font-size: var(--text-figures);
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

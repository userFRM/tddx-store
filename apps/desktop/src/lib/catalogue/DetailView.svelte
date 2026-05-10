<script lang="ts">
  import {
    ArrowLeft,
    ChevronDown,
    Plus,
    Check,
    ExternalLink,
  } from "lucide-svelte";
  import {
    app,
    navigate,
    openComposer,
    type DatasetMeta,
  } from "$lib/stores/app.svelte";
  import { api, fmtBytes, fmtNum, type Coverage } from "$lib/api";
  import { onMount } from "svelte";
  import CoverageHeatmap from "$lib/queue/CoverageHeatmap.svelte";
  import { renderMarkdown } from "$lib/util/md";

  type DetailTab = "schema" | "sample" | "coverage" | "settings";

  let activeTab = $state<DetailTab>("schema");

  // Coverage data from API
  let coverage = $state<Coverage[]>([]);
  let loadingCoverage = $state(false);

  // Sample data placeholder rows
  let sampleRows = $state<Record<string, string>[]>([]);
  let loadingSample = $state(false);
  let sampleError = $state("");

  const dataset = $derived(app.detailDataset!);
  const catalogueEntry = $derived(
    dataset ? app.catalogue.find((e) => e.name === dataset.id) ?? null : null
  );

  function goBack() {
    navigate("browse");
    activeTab = "schema";
  }

  function switchTab(t: DetailTab) {
    activeTab = t;
    if (t === "coverage" && coverage.length === 0 && !loadingCoverage) {
      fetchCoverage();
    }
  }

  async function fetchCoverage() {
    loadingCoverage = true;
    try {
      const all = await api.coverage();
      coverage = all.filter(c => c.kind === dataset.id);
    } catch {
      // not connected
    } finally {
      loadingCoverage = false;
    }
  }

  onMount(() => {
    if (activeTab === "coverage") fetchCoverage();
  });
</script>

{#if dataset}
<div class="detail">
  <!-- Back nav -->
  <button class="back-btn" onclick={goBack} aria-label="Back to browse">
    <ArrowLeft size={14} strokeWidth={1.75} />
    Browse
  </button>

  <!-- Hero -->
  <div class="detail-hero">
    <div class="hero-left">
      <div class="hero-meta text-caption">
        {dataset.assetClass.toUpperCase()} · {dataset.cadence.toUpperCase().replace("_", "-")}
      </div>
      <h1 class="hero-title">{dataset.title}</h1>
      <p class="hero-subtitle">{dataset.subtitle}</p>
      <div class="spec-mono">{dataset.specLine}</div>
    </div>

    <div class="hero-actions">
      <button
        class="btn btn-primary add-btn"
        onclick={() => openComposer(dataset)}
        aria-label="Add {dataset.title} to queue"
      >
        <Plus size={14} strokeWidth={1.75} />
        Add to Queue
        <span class="add-btn-divider" aria-hidden="true"></span>
        <ChevronDown size={12} strokeWidth={1.75} />
      </button>
    </div>
  </div>

  <!-- Tabs -->
  <nav class="tab-bar" role="tablist">
    {#each (["schema", "sample", "coverage", "settings"] as DetailTab[]) as tab}
      <button
        role="tab"
        class="tab-btn"
        class:active={activeTab === tab}
        aria-selected={activeTab === tab}
        onclick={() => switchTab(tab)}
      >
        {tab.charAt(0).toUpperCase() + tab.slice(1)}
      </button>
    {/each}
  </nav>

  <!-- Tab content -->
  <div class="tab-content" role="tabpanel">

    {#if activeTab === "schema"}
      <div class="schema-view">
        {#if catalogueEntry}
          <div class="schema-intro markdown-body">
            {@html renderMarkdown(catalogueEntry.description)}
          </div>
          <div class="schema-params-note">
            <p class="text-body-sm fg-muted">
              Field-level schema is sourced from the downloaded file.
              Open a file in the Data Viewer (Library view) to inspect column types and values.
            </p>
            <div class="params-table-wrap">
              <table class="data-table" aria-label="Parameters for {dataset.title}">
                <thead>
                  <tr>
                    <th>Parameter</th>
                    <th>Type</th>
                    <th>Required</th>
                    <th>Description</th>
                  </tr>
                </thead>
                <tbody>
                  {#each catalogueEntry.params as p (p.name)}
                    <tr>
                      <td><code class="field-name">{p.name}</code></td>
                      <td><span class="type-badge">{p.param_type}</span></td>
                      <td>
                        {#if p.required}
                          <span class="nullable-no">yes</span>
                        {:else}
                          <span class="nullable-yes">optional</span>
                        {/if}
                      </td>
                      <td class="desc-col markdown-body">
                        {@html renderMarkdown(p.description)}
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          </div>
        {:else}
          <div class="schema-intro">
            <p class="text-body-sm fg-muted">
              Field-level schema will appear here once the catalogue is loaded.
            </p>
          </div>
        {/if}
      </div>

    {:else if activeTab === "sample"}
      <div class="sample-view">
        <div class="sample-notice">
          <div class="notice-icon" aria-hidden="true">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <circle cx="8" cy="8" r="7" stroke="var(--fg-subtle)" stroke-width="1.5" />
              <path d="M8 5v4M8 11v.5" stroke="var(--fg-subtle)" stroke-width="1.5" stroke-linecap="round" />
            </svg>
          </div>
          <div>
            <p class="text-body-sm">
              Sample data requires an active ThetaData connection and a valid symbol.
              Queue a download from the composer to get real data locally.
            </p>
            <button
              class="btn btn-secondary"
              style="margin-top: var(--sp-3);"
              onclick={() => openComposer(dataset)}
            >
              <Plus size={14} strokeWidth={1.75} />
              Queue a download
            </button>
          </div>
        </div>

        {#if sampleError}
          <p class="sample-error">{sampleError}</p>
        {/if}
      </div>

    {:else if activeTab === "coverage"}
      <div class="coverage-view">
        <!-- Live calendar heatmap: upstream availability vs local files -->
        <CoverageHeatmap symbol="QQQ" kind={dataset.id} />
        <div class="divider"></div>
        {#if loadingCoverage}
          <div class="loading-state">
            <div class="spinner" aria-label="Loading coverage data"></div>
            <span class="fg-muted text-body-sm">Loading coverage…</span>
          </div>
        {:else if coverage.length === 0}
          <div class="empty-state">
            <p class="text-body-sm fg-muted">
              No local data yet for <strong>{dataset.title}</strong>.
              Queue a download to start building your library.
            </p>
            <button
              class="btn btn-primary"
              style="margin-top: var(--sp-4);"
              onclick={() => openComposer(dataset)}
            >
              <Plus size={14} strokeWidth={1.75} />
              Queue download
            </button>
          </div>
        {:else}
          <div class="coverage-list">
            {#each coverage as cov (cov.symbol)}
              <div class="coverage-row">
                <div class="cov-symbol text-mono">{cov.symbol}</div>
                <div class="cov-stats">
                  <span class="text-caption">{fmtNum(cov.files)} files</span>
                  <span class="cov-sep">·</span>
                  <span class="text-caption">{fmtBytes(cov.bytes)}</span>
                  <span class="cov-sep">·</span>
                  <span class="text-caption text-mono">{cov.first ?? "—"} → {cov.last ?? "—"}</span>
                </div>
                <button
                  class="btn btn-ghost cov-action"
                  onclick={() => openComposer(dataset)}
                >
                  <Plus size={12} strokeWidth={1.75} />
                  Add dates
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>

    {:else if activeTab === "settings"}
      <div class="settings-tab">
        <p class="text-body-sm fg-muted" style="margin-bottom: var(--sp-5);">
          Per-dataset defaults. Applies when using the quick "Add to Queue" button.
        </p>
        <div class="settings-group">
          <div class="settings-row">
            <label class="settings-label" for="ds-format">Default format</label>
            <select id="ds-format" class="field-input" style="width: 200px;">
              <option value="parquet">parquet (zstd compressed)</option>
              <option value="csv">csv</option>
              <option value="jsonl">jsonl (newline-delimited)</option>
              <option value="json">json</option>
            </select>
          </div>
        </div>
        <p class="text-caption" style="margin-top: var(--sp-4);">
          Global settings (output dir, workers, credentials) are in the Settings view.
        </p>
      </div>
    {/if}

  </div>
</div>
{/if}

<style>
  .detail {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow-y: auto;
    padding: var(--sp-6) var(--sp-8);
    gap: var(--sp-6);
  }

  /* Back */
  .back-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    background: none;
    border: none;
    cursor: pointer;
    padding: var(--sp-1) 0;
    transition: color var(--dur-fast) var(--ease-standard);
    width: fit-content;
    outline: none;
  }
  .back-btn:hover { color: var(--accent); }
  .back-btn:focus-visible { color: var(--accent); }

  /* Hero */
  .detail-hero {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-6);
  }

  .hero-left {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }

  .hero-meta {
    font-size: var(--text-caption);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--fg-subtle);
    font-weight: var(--weight-medium);
  }

  .hero-title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    letter-spacing: -0.015em;
    color: var(--fg);
    line-height: 1.15;
  }

  .hero-subtitle {
    font-size: var(--text-body);
    color: var(--fg-muted);
    max-width: 480px;
  }

  .spec-mono {
    font-family: var(--font-mono);
    font-size: 0.8125rem;
    font-variant-numeric: tabular-nums;
    color: var(--fg-subtle);
    line-height: 1.4;
  }

  /* Add button */
  .hero-actions {
    flex-shrink: 0;
    margin-top: var(--sp-2);
  }

  .add-btn {
    height: 36px;
    padding: 0 var(--sp-4);
    font-size: var(--text-body-sm);
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
  }

  .add-btn-divider {
    width: 1px;
    height: 14px;
    background: rgba(10, 12, 20, 0.35);
    margin: 0 var(--sp-1);
  }

  /* Tabs */
  .tab-bar {
    display: flex;
    gap: 0;
    border-bottom: 1px solid var(--border);
  }

  .tab-btn {
    padding: var(--sp-2) var(--sp-4);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    color: var(--fg-muted);
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    transition:
      color var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard);
    outline: none;
    margin-bottom: -1px;
  }
  .tab-btn:hover { color: var(--fg); }
  .tab-btn:focus-visible { box-shadow: inset var(--shadow-glow-accent); }
  .tab-btn.active {
    color: var(--fg);
    border-bottom-color: var(--accent);
  }

  /* Tab content */
  .tab-content {
    flex: 1;
    min-height: 0;
  }

  /* Schema table */
  .schema-view {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }

  .schema-intro { padding: var(--sp-1) 0; }

  .schema-params-note {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }

  .params-table-wrap {
    overflow-x: auto;
  }

  .data-table {
    width: 100%;
    border-collapse: collapse;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    overflow: hidden;
  }

  .data-table thead tr {
    background: var(--surface-2);
  }

  .data-table thead th {
    padding: var(--sp-2) var(--sp-4);
    text-align: left;
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--fg-muted);
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }

  .data-table tbody td {
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--border);
    font-size: var(--text-body-sm);
    vertical-align: top;
    color: var(--fg);
  }

  .data-table tbody tr:last-child td { border-bottom: none; }
  .data-table tbody tr:hover td { background: var(--surface-2); }

  .field-name {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    color: var(--fg);
    background: var(--surface-2);
    padding: 1px var(--sp-2);
    border-radius: var(--r-sm);
  }

  .type-badge {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    font-variant-numeric: tabular-nums;
    font-weight: var(--weight-medium);
  }

  .nullable-yes { color: var(--fg-subtle); font-size: var(--text-body-sm); }
  .nullable-no  { color: var(--fg-muted);  font-size: var(--text-body-sm); font-weight: var(--weight-medium); }

  .desc-col { color: var(--fg-muted); max-width: 300px; }

  /* Sample view */
  .sample-view {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }

  .sample-notice {
    display: flex;
    gap: var(--sp-4);
    align-items: flex-start;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    padding: var(--sp-5);
    max-width: 600px;
  }

  .notice-icon { flex-shrink: 0; margin-top: 2px; }

  .sample-error { color: var(--bad); font-size: var(--text-body-sm); }

  /* Coverage view */
  .coverage-view {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }

  .coverage-list {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    overflow: hidden;
  }

  .coverage-row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--border);
    transition: background var(--dur-fast) var(--ease-standard);
  }
  .coverage-row:last-child { border-bottom: none; }
  .coverage-row:hover { background: var(--surface-2); }

  .cov-symbol {
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    min-width: 80px;
  }

  .cov-stats {
    flex: 1;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--text-caption);
    color: var(--fg-muted);
    flex-wrap: wrap;
  }

  .cov-sep { color: var(--fg-subtle); }

  .cov-action {
    height: 26px;
    font-size: 0.75rem;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-standard);
  }
  .coverage-row:hover .cov-action { opacity: 1; }

  /* Loading / empty states */
  .loading-state {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-8);
  }

  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  .empty-state {
    padding: var(--sp-8);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    max-width: 480px;
  }

  /* Settings tab */
  .settings-tab {
    padding: var(--sp-2) 0;
    max-width: 520px;
  }

  .settings-group {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    overflow: hidden;
  }

  .settings-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--sp-4);
    border-bottom: 1px solid var(--border);
  }
  .settings-row:last-child { border-bottom: none; }

  .settings-label {
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    color: var(--fg);
  }
</style>

<script lang="ts">
  /**
   * DatasetRow — one expandable row in the dataset catalogue list.
   *
   * Collapsed: single 52px line with summary, tag chip, tier pill, param count, chevron.
   * Expanded:  inline panel with full markdown description, parameters table,
   *            returns line, REST path, and footer actions.
   *
   * Selection and expansion are independent:
   *   - Clicking anywhere on the collapsed row (except the chevron) expands it
   *     but does NOT select it — the user is just browsing.
   *   - Clicking the chevron only toggles expand state.
   *   - Clicking "Use this dataset" in the expanded footer sets selected + collapses.
   *   - ESC while expanded collapses the row.
   */
  import { ChevronDown, Lock, ArrowUpRight, Database } from "lucide-svelte";
  import { type CatalogueEntry, type TierName, TIER_RANK } from "$lib/api";
  import { app, tierForKind } from "$lib/stores/app.svelte";
  import { renderMarkdown } from "$lib/util/md";
  import { openUrl } from "@tauri-apps/plugin-opener";

  let {
    entry,
    selected = false,
    onselect,
  }: {
    entry: CatalogueEntry;
    selected: boolean;
    onselect: (name: string) => void;
  } = $props();

  let expanded = $state(false);

  // ── Derived tier state ────────────────────────────────────────
  const effectiveTier = $derived<TierName>(
    entry.min_tier ?? tierForKind(entry.name).required
  );

  const gated = $derived(
    !!app.tierStatus &&
    !tierForKind(entry.name).allowed &&
    TIER_RANK[effectiveTier] > TIER_RANK["Free"]
  );

  // ── Markdown rendering (cached on demand) ─────────────────────
  const descriptionHtml = $derived(renderMarkdown(entry.description));

  // ── Computed display values ───────────────────────────────────
  const requiredCount = $derived(
    entry.params.filter((p) => p.required).length
  );

  const tagDisplay = $derived(
    entry.tag || entry.subcategory.replace(/_/g, " ")
  );

  // Several endpoints share the same `summary` (e.g. "Trade" exists
  // under history + at_time subcategories — snapshot is filtered
  // upstream). Always append a subcategory suffix so rows are
  // unambiguous; the tag chip groups visually but the suffix tells
  // the user WHICH variant they're looking at.
  function subcategoryLabel(sc: string): string {
    switch (sc) {
      case "history": return "History";
      case "history_eod": return "History · EOD";
      case "history_greeks": return "History · Greeks";
      case "history_greeks_eod": return "History · Greeks EOD";
      case "list": return "List";
      case "at_time": return "At time";
      default: return sc.replace(/_/g, " ") || "—";
    }
  }
  const subLabel = $derived(subcategoryLabel(entry.subcategory));
  const titleText = $derived(
    subLabel
      ? `${entry.summary || entry.name} · ${subLabel}`
      : entry.summary || entry.name,
  );

  // ── Interaction handlers ──────────────────────────────────────
  function toggleExpand(e: MouseEvent) {
    e.stopPropagation();
    expanded = !expanded;
  }

  function handleRowClick() {
    expanded = !expanded;
  }

  function handleRowKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      expanded = !expanded;
    }
    if (e.key === "Escape" && expanded) {
      expanded = false;
    }
  }

  function handlePanelKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      expanded = false;
    }
  }

  function handleUse() {
    onselect(entry.name);
    expanded = false;
  }

  async function handleUpgrade() {
    const url = app.tierStatus?.upgrade_url ?? "https://thetadata.net/pricing";
    try { await openUrl(url); } catch { /* swallow */ }
  }

  function tierClass(t: TierName): string {
    return "tier-pill tier-" + t.toLowerCase();
  }
</script>

<div
  class="dataset-row"
  class:selected
  class:gated
  class:expanded
  role="row"
>
  <!-- ── Collapsed header bar ──────────────────────────────── -->
  <div
    class="row-header"
    role="button"
    tabindex={0}
    aria-expanded={expanded}
    aria-label="{entry.summary || entry.name}{gated ? ` — requires ${effectiveTier}` : ''}"
    onclick={handleRowClick}
    onkeydown={handleRowKeydown}
  >
    <!-- Status dot -->
    <span
      class="status-dot"
      aria-hidden="true"
      class:active={selected}
    ></span>

    <!-- Summary title (with subcategory suffix when ambiguous) -->
    <span class="row-summary" title={titleText}>
      {entry.summary || entry.name}
      {#if subLabel}
        <span class="row-sub-label">· {subLabel}</span>
      {/if}
    </span>

    <!-- Tag chip -->
    <span class="tag-chip" aria-label="Group: {tagDisplay}">
      {tagDisplay}
    </span>

    <!-- Tier pill + lock -->
    <span class={tierClass(effectiveTier)} aria-label="Tier: {effectiveTier}">
      {#if gated}
        <Lock size={10} strokeWidth={1.75} aria-hidden="true" />
      {/if}
      {effectiveTier}
    </span>

    <!-- Param count -->
    <span class="param-count tabnum" aria-label="{entry.params.length} parameters">
      {entry.params.length}p
    </span>

    <!-- Chevron toggle — separate click target so row click and chevron
         click both work cleanly -->
    <button
      type="button"
      class="chevron-btn btn-icon"
      aria-label={expanded ? "Collapse" : "Expand"}
      tabindex={-1}
      onclick={toggleExpand}
    >
      <ChevronDown
        size={14}
        strokeWidth={1.75}
        class="chevron-icon"
        style="transform: rotate({expanded ? '180deg' : '0deg'}); transition: transform var(--dur-base) var(--ease-standard);"
      />
    </button>
  </div>

  <!-- ── Expanded panel ─────────────────────────────────────── -->
  {#if expanded}
    <!-- tabindex="-1": makes the panel programmatically focusable so the
         browser will route keyboard events here (required for ESC handling),
         without placing it in the natural tab order. -->
    <section
      class="row-panel"
      aria-label="Details for {entry.summary || entry.name}"
      tabindex="-1"
      onkeydown={handlePanelKeydown}
    >
      <!-- Description -->
      {#if entry.description}
        <div class="panel-section">
          <div class="markdown-body">
            {@html descriptionHtml}
          </div>
        </div>
      {/if}

      <!-- Parameters table -->
      {#if entry.params.length > 0}
        <div class="panel-section">
          <div class="section-label">Parameters</div>
          <table class="params-table" aria-label="Parameters for {entry.summary}">
            <thead>
              <tr>
                <th scope="col">Name</th>
                <th scope="col">Type</th>
                <th scope="col">Required</th>
                <th scope="col">Description</th>
              </tr>
            </thead>
            <tbody>
              {#each entry.params as param (param.name)}
                <tr class:required-row={param.required}>
                  <td class="param-name tabnum">{param.name}</td>
                  <td class="param-type">{param.param_type}</td>
                  <td class="param-req">
                    {#if param.required}
                      <span class="req-badge" aria-label="required">req</span>
                    {:else}
                      <span class="opt-badge" aria-label="optional">opt</span>
                    {/if}
                  </td>
                  <td class="param-desc">{param.description || "—"}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}

      <!-- Returns + REST path -->
      <div class="panel-meta-row">
        {#if entry.returns}
          <div class="meta-item">
            <span class="meta-label">Returns</span>
            <span class="meta-value">{entry.returns}</span>
          </div>
        {/if}
        {#if entry.rest_path}
          <div class="meta-item">
            <span class="meta-label">Endpoint</span>
            <code class="mono-path">{entry.rest_path}</code>
          </div>
        {/if}
      </div>

      <!-- Footer actions -->
      <div class="panel-footer">
        <div class="panel-entry-meta">
          <Database size={12} strokeWidth={1.75} aria-hidden="true" />
          <span class="entry-name tabnum">{entry.name}</span>
          {#if requiredCount > 0}
            <span class="req-count-note">
              {requiredCount} required param{requiredCount === 1 ? "" : "s"}
            </span>
          {/if}
        </div>

        <div class="panel-actions">
          {#if gated}
            <button
              type="button"
              class="btn btn-secondary upgrade-action-btn"
              onclick={handleUpgrade}
              aria-label="Upgrade subscription to {effectiveTier} to access this dataset"
            >
              <ArrowUpRight size={14} strokeWidth={1.75} />
              Upgrade to {effectiveTier}
            </button>
          {:else}
            <button
              type="button"
              class="btn btn-primary use-btn"
              class:already-selected={selected}
              onclick={handleUse}
              aria-label="Use {entry.summary || entry.name}"
            >
              {selected ? "Selected" : "Use this dataset"}
            </button>
          {/if}
        </div>
      </div>
    </section>
  {/if}
</div>

<style>
  /* ── Row shell ──────────────────────────────────────────────── */
  .dataset-row {
    display: flex;
    flex-direction: column;
    border-bottom: 1px solid var(--border);
    background: var(--surface-1);
    border-left: 3px solid transparent;
    transition:
      border-left-color var(--dur-fast) var(--ease-standard),
      background var(--dur-fast) var(--ease-standard);
  }

  .dataset-row:last-child {
    border-bottom: none;
  }

  .dataset-row.selected {
    border-left-color: var(--accent);
    background: color-mix(in srgb, var(--surface-1) 92%, var(--accent) 8%);
  }

  .dataset-row.gated {
    opacity: 0.65;
    filter: grayscale(0.4);
  }

  /* ── Collapsed header bar ───────────────────────────────────── */
  .row-header {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 0 var(--sp-4);
    min-height: 52px;
    cursor: pointer;
    outline: none;
    user-select: none;
  }

  .row-header:hover {
    background: var(--surface-2);
  }

  .dataset-row.expanded .row-header {
    background: var(--surface-2);
  }

  .row-header:focus-visible {
    box-shadow: inset 0 0 0 2px var(--accent);
  }

  /* ── Status dot ─────────────────────────────────────────────── */
  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--border-strong);
    flex-shrink: 0;
    transition: background var(--dur-fast) var(--ease-standard);
  }

  .status-dot.active {
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-tint);
  }

  /* ── Summary ────────────────────────────────────────────────── */
  .row-summary {
    flex: 1;
    font-size: var(--text-body);
    font-weight: var(--weight-medium);
    color: var(--fg);
    letter-spacing: -0.005em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .dataset-row.gated .row-summary {
    color: var(--fg-muted);
  }

  .row-sub-label {
    font-weight: var(--weight-normal);
    color: var(--fg-subtle);
    margin-left: 2px;
    font-size: 0.95em;
  }

  /* ── Tag chip ───────────────────────────────────────────────── */
  .tag-chip {
    flex-shrink: 0;
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    color: var(--fg-subtle);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    padding: 2px var(--sp-2);
    letter-spacing: 0.03em;
    text-transform: uppercase;
    white-space: nowrap;
  }

  /* ── Tier pill ──────────────────────────────────────────────── */
  .tier-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    padding: 2px 7px;
    border-radius: var(--r-pill);
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    border: 1px solid transparent;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .tier-unknown  { background: var(--surface-2);                      color: var(--fg-subtle);       }
  .tier-free     { background: rgba(92, 101, 119, 0.15);              color: var(--fg-muted);        }
  .tier-value    { background: rgba(56, 132, 255, 0.10);  color: rgb(56, 132, 255);  border-color: rgba(56, 132, 255, 0.20); }
  .tier-standard { background: rgba(34, 175, 109, 0.12);  color: rgb(34, 175, 109); border-color: rgba(34, 175, 109, 0.22); }
  .tier-pro      { background: rgba(244, 196, 48, 0.14);  color: rgb(212, 158, 0);  border-color: rgba(244, 196, 48, 0.30); }

  /* ── Param count ────────────────────────────────────────────── */
  .param-count {
    flex-shrink: 0;
    font-size: var(--text-caption);
    color: var(--fg-subtle);
    min-width: 24px;
    text-align: right;
  }

  /* ── Chevron button ─────────────────────────────────────────── */
  .chevron-btn {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    background: transparent;
    border: none;
    color: var(--fg-subtle);
    border-radius: var(--r-sm);
    cursor: pointer;
    transition: color var(--dur-fast) var(--ease-standard),
                background var(--dur-fast) var(--ease-standard);
  }

  .chevron-btn:hover {
    background: var(--surface-3);
    color: var(--fg);
  }

  /* ── Expanded panel ─────────────────────────────────────────── */
  .row-panel {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-5) var(--sp-5);
    border-top: 1px solid var(--border);
    background: var(--surface-1);
  }

  /* ── Panel section ──────────────────────────────────────────── */
  .panel-section {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .section-label {
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    color: var(--fg-subtle);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  /* ── Parameters table ───────────────────────────────────────── */
  .params-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--text-body-sm);
  }

  .params-table th {
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    color: var(--fg-subtle);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    text-align: left;
    padding: var(--sp-2) var(--sp-3) var(--sp-2) 0;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }

  .params-table td {
    padding: var(--sp-2) var(--sp-3) var(--sp-2) 0;
    vertical-align: top;
    border-bottom: 1px solid var(--border);
    color: var(--fg-muted);
    line-height: 1.45;
  }

  .params-table tbody tr:last-child td {
    border-bottom: none;
  }

  .params-table tbody tr:hover td {
    color: var(--fg);
  }

  .param-name {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    color: var(--fg) !important;
    white-space: nowrap;
    width: 1%;
    padding-right: var(--sp-4) !important;
  }

  .param-type {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    color: var(--fg-muted) !important;
    white-space: nowrap;
    width: 1%;
    padding-right: var(--sp-4) !important;
  }

  .param-req {
    width: 1%;
    white-space: nowrap;
    padding-right: var(--sp-4) !important;
  }

  .param-desc {
    color: var(--fg-muted);
    line-height: 1.45;
  }

  .req-badge {
    display: inline-flex;
    padding: 1px 5px;
    border-radius: var(--r-pill);
    font-size: 10px;
    font-weight: var(--weight-medium);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    background: rgba(124, 140, 255, 0.12);
    color: var(--accent);
    border: 1px solid rgba(124, 140, 255, 0.22);
  }

  .opt-badge {
    display: inline-flex;
    padding: 1px 5px;
    border-radius: var(--r-pill);
    font-size: 10px;
    font-weight: var(--weight-medium);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    background: var(--surface-2);
    color: var(--fg-subtle);
    border: 1px solid var(--border);
  }

  /* ── Meta row (returns + path) ──────────────────────────────── */
  .panel-meta-row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-5);
    padding: var(--sp-3) var(--sp-4);
    background: var(--surface-2);
    border-radius: var(--r-sm);
    border: 1px solid var(--border);
  }

  .meta-item {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
    min-width: 0;
  }

  .meta-label {
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    color: var(--fg-subtle);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    flex-shrink: 0;
  }

  .meta-value {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mono-path {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    color: var(--fg-muted);
    background: transparent;
    padding: 0;
    border-radius: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* ── Panel footer ───────────────────────────────────────────── */
  .panel-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }

  .panel-entry-meta {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    color: var(--fg-subtle);
    min-width: 0;
    overflow: hidden;
  }

  .entry-name {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    color: var(--fg-subtle);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .req-count-note {
    font-size: var(--text-caption);
    color: var(--fg-subtle);
    white-space: nowrap;
    letter-spacing: 0.03em;
  }

  .req-count-note::before {
    content: "·";
    margin-right: var(--sp-2);
  }

  /* ── Action buttons ─────────────────────────────────────────── */
  .panel-actions {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex-shrink: 0;
  }

  .use-btn {
    height: 32px;
    padding: 0 var(--sp-4);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    border-radius: var(--r-sm);
  }

  .use-btn.already-selected {
    background: var(--accent);
    color: #fff;
    cursor: default;
    box-shadow: inset 0 0 0 2px rgba(255, 255, 255, 0.25);
  }

  .upgrade-action-btn {
    height: 32px;
    padding: 0 var(--sp-3);
    font-size: var(--text-body-sm);
  }
</style>

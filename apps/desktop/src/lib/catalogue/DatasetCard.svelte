<script lang="ts">
  import { Plus, Check, MoreHorizontal, TrendingUp, BarChart2, Activity, Layers, Lock, ArrowUpRight } from "lucide-svelte";
  import type { DatasetMeta } from "$lib/stores/app.svelte";
  import { app, openComposer, openDetail as openDatasetDetail, tierForKind } from "$lib/stores/app.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";

  let { dataset, queued = false }: { dataset: DatasetMeta; queued?: boolean } = $props();

  // Tier verdict for this dataset. `required` is always known
  // (driven by `minTierForKind`, a static client-side mirror) so we
  // can show the requirement even pre-connect. `gated` flips to true
  // either when we know the user's tier and they're below it, OR when
  // they're not connected — in both cases we want a visible lock so
  // the user understands the barrier before they queue.
  const verdict = $derived(tierForKind(dataset.id));
  const gated = $derived(!verdict.allowed);
  const showUpgrade = $derived(!!app.tierStatus && !verdict.allowed);
  const upgradeUrl = $derived(app.tierStatus?.upgrade_url ?? "https://thetadata.net/pricing");

  async function handleUpgrade(e: MouseEvent) {
    e.stopPropagation();
    try { await openUrl(upgradeUrl); } catch {}
  }

  function assetIcon(assetClass: DatasetMeta["assetClass"]) {
    switch (assetClass) {
      case "stock":  return TrendingUp;
      case "option": return BarChart2;
      case "index":  return Layers;
      case "rate":   return Activity;
    }
  }

  function cadenceLabel(c: DatasetMeta["cadence"]) {
    switch (c) {
      case "trade":       return "Trade tick";
      case "quote":       return "Quote tick";
      case "trade_quote": return "Trade-Quote";
      case "eod":         return "EOD";
      case "snapshot":    return "Snapshot";
      case "greeks":      return "Greeks";
      case "oi":          return "Open Interest";
    }
  }

  function assetLabel(a: DatasetMeta["assetClass"]) {
    switch (a) {
      case "stock":  return "Equity";
      case "option": return "Options";
      case "index":  return "Index";
      case "rate":   return "Rates";
    }
  }

  function openDetail() {
    openDatasetDetail(dataset);
  }

  function handleQueue(e: MouseEvent) {
    e.stopPropagation();
    openComposer(dataset);
  }
</script>

<article
  class="card"
  class:queued
  class:gated
  role="button"
  tabindex="0"
  aria-label={gated
    ? `${dataset.title} — requires ${verdict.required} tier`
    : `Open ${dataset.title}`}
  onclick={openDetail}
  onkeydown={(e) => e.key === "Enter" && openDetail()}
>
  <!-- Header row: asset class + cadence label + tier pill -->
  <div class="card-header">
    <div class="asset-badge">
      <svelte:component this={assetIcon(dataset.assetClass)} size={12} strokeWidth={1.75} />
      <span>{assetLabel(dataset.assetClass)}</span>
    </div>
    <span class="cadence-tag">{cadenceLabel(dataset.cadence)}</span>
    {#if gated}
      <span class="tier-lock" title={`Requires ${verdict.required} (you have ${verdict.user})`}>
        <Lock size={10} strokeWidth={2} />
        <span>{verdict.required}</span>
      </span>
    {/if}
  </div>

  <!-- Title + subtitle -->
  <div class="card-body">
    <h3 class="card-title">{dataset.title}</h3>
    <p class="card-subtitle">{dataset.subtitle}</p>
  </div>

  <!-- Spec line -->
  <div class="spec-line">{dataset.specLine}</div>

  <!-- Actions row -->
  <div class="card-actions">
    {#if queued}
      <button class="btn-queued" disabled>
        <Check size={12} strokeWidth={2} />
        In queue
      </button>
    {:else if gated && showUpgrade}
      <button
        class="btn-upgrade"
        onclick={handleUpgrade}
        aria-label="Upgrade ThetaData subscription to access {dataset.title}"
      >
        <ArrowUpRight size={12} strokeWidth={2} />
        Upgrade to {verdict.required}
      </button>
    {:else if gated}
      <button
        class="btn btn-ghost queue-btn"
        onclick={handleQueue}
        aria-label="Sign in to queue {dataset.title}"
      >
        <Lock size={12} strokeWidth={2} />
        Sign in to check
      </button>
    {:else}
      <button
        class="btn btn-primary queue-btn"
        onclick={handleQueue}
        aria-label="Add {dataset.title} to queue"
      >
        <Plus size={12} strokeWidth={2} />
        Queue
      </button>
    {/if}
    <button
      class="btn-icon more-btn"
      onclick={(e) => { e.stopPropagation(); openDetail(); }}
      aria-label="More options for {dataset.title}"
    >
      <MoreHorizontal size={14} strokeWidth={1.75} />
    </button>
  </div>
</article>

<style>
  .card {
    /* Fixed width AND height so every shelf tile aligns. The body grows
       within these bounds; long subtitles wrap to two lines max. */
    width: 280px;
    height: 220px;
    flex-shrink: 0;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    padding: var(--sp-4);
    display: grid;
    grid-template-rows: auto 1fr auto auto;
    gap: var(--sp-3);
    cursor: pointer;
    transition:
      background var(--dur-base) var(--ease-standard),
      border-color var(--dur-base) var(--ease-standard),
      transform var(--dur-fast) var(--ease-standard),
      box-shadow var(--dur-fast) var(--ease-standard);
    outline: none;
    position: relative;
  }

  .card:hover {
    background: var(--surface-2);
    border-color: var(--border-strong);
    transform: translateY(-1px);
  }

  .card:focus-visible {
    box-shadow: var(--shadow-glow-accent);
    border-color: var(--accent);
  }

  .card.queued {
    border-left: 3px solid var(--accent);
    box-shadow: -2px 0 12px var(--accent-tint);
  }

  /* Tier-gated: card stays clickable (opens detail) but visually
     desaturated + slightly faded so the eye skips it in shelf scans.
     Title/subtitle keep readable contrast — only chrome dims. The
     Upgrade button + tier-lock pill stay full-opacity below. */
  .card.gated {
    opacity: 0.62;
    filter: grayscale(0.55);
    background: var(--surface-2);
    border-style: dashed;
  }
  .card.gated:hover {
    opacity: 0.92;
    filter: grayscale(0.20);
  }
  /* Don't dim the locked indicator or the upgrade CTA — those are the
     actionable bits we want loud. */
  .card.gated :global(.tier-lock),
  .card.gated :global(.btn-upgrade) {
    opacity: 1;
    filter: none;
  }

  /* Header */
  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
  }

  .asset-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--fg-muted);
  }

  .cadence-tag {
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--fg-subtle);
  }

  /* Body */
  .card-body {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    flex: 1;
  }

  .card-title {
    font-size: var(--text-heading);
    font-weight: var(--weight-semi);
    color: var(--fg);
    letter-spacing: -0.005em;
    line-height: 1.3;
  }

  .card-subtitle {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    line-height: 1.45;
    /* Clamp to two lines so cards never grow past the fixed height. */
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Spec line */
  .spec-line {
    font-family: var(--font-mono);
    font-size: var(--text-caption);
    font-variant-numeric: tabular-nums;
    color: var(--fg-subtle);
    line-height: 1.4;
    border-top: 1px solid var(--border);
    padding-top: var(--sp-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Actions */
  .card-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
  }

  .queue-btn {
    height: 28px;
    padding: 0 var(--sp-3);
    font-size: var(--text-body-sm);
  }

  .btn-queued {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 28px;
    padding: 0 var(--sp-3);
    border-radius: var(--r-sm);
    background: var(--accent-tint);
    color: var(--accent-hi);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    border: 1px solid rgba(124, 140, 255, 0.2);
    cursor: default;
  }

  .more-btn {
    width: 28px;
    height: 28px;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-standard);
  }

  .card:hover .more-btn,
  .card:focus-within .more-btn {
    opacity: 1;
  }

  /* Tier-locked indicator (header right of cadence-tag). */
  .tier-lock {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 2px 6px;
    border-radius: 999px;
    background: rgba(244, 196, 48, 0.14);
    color: rgb(212, 158, 0);
    border: 1px solid rgba(244, 196, 48, 0.32);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  /* Upgrade CTA replaces the Queue button when verdict.allowed=false. */
  .btn-upgrade {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 28px;
    padding: 0 var(--sp-3);
    border-radius: var(--r-sm);
    border: 1px solid var(--accent, rgb(56, 132, 255));
    background: var(--accent, rgb(56, 132, 255));
    color: white;
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    cursor: pointer;
    transition: filter var(--dur-fast) var(--ease-standard);
  }
  .btn-upgrade:hover { filter: brightness(1.08); }
  .btn-upgrade:active { filter: brightness(0.95); }
  @media (prefers-reduced-motion: reduce) {
    .btn-upgrade { transition: none; }
  }
</style>

<script lang="ts">
  /**
   * Step 1 — What asset class?
   * Four tiles: Stocks · Options · Indices · Rates.
   * Shows the user's tier per tile, sourced from app.tierStatus.
   * Cross-asset suggestions (e.g. "also queue options chain for QQQ?")
   * fire AFTER a queue submit, not as a unified tile here.
   */
  import { TrendingUp, BarChart2, LineChart, Percent } from "lucide-svelte";
  import { app } from "$lib/stores/app.svelte";
  import type { AssetClass } from "$lib/stores/app.svelte";
  import type { TierName } from "$lib/api";

  let {
    value = $bindable<AssetClass>("stock"),
  }: {
    value: AssetClass;
  } = $props();

  type TileSpec = {
    id: AssetClass;
    label: string;
    description: string;
    icon: typeof TrendingUp;
    tierKey: keyof NonNullable<typeof app.tierStatus>;
  };

  const tiles: TileSpec[] = [
    { id: "stock",  label: "Stocks",  description: "NMS equities — trades, quotes, OHLC, EOD", icon: TrendingUp,  tierKey: "stock" },
    { id: "option", label: "Options", description: "Full option chain — trades, quotes, greeks, OI", icon: BarChart2, tierKey: "options" },
    { id: "index",  label: "Indices", description: "Index levels and OHLC bars",                icon: LineChart,  tierKey: "indices" },
    { id: "rate",   label: "Rates",   description: "Treasury and SOFR benchmark levels",        icon: Percent,    tierKey: "interest_rate" },
  ];

  function tierDisplay(t: TierName): string {
    return t === "Unknown" ? "—" : t;
  }

  function tierClass(t: TierName): string {
    if (t === "Pro")      return "tier-pro";
    if (t === "Standard") return "tier-standard";
    if (t === "Value")    return "tier-value";
    if (t === "Free")     return "tier-free";
    return "tier-unknown";
  }

  const status = $derived(app.tierStatus);
</script>

<div class="asset-grid" role="radiogroup" aria-label="Asset class">
  {#each tiles as tile}
    {@const Icon = tile.icon}
    {@const userTier = (status?.[tile.tierKey] ?? "Unknown") as TierName}
    <button
      type="button"
      role="radio"
      aria-checked={value === tile.id}
      class="tile"
      class:selected={value === tile.id}
      onclick={() => (value = tile.id)}
    >
      <div class="tile-header">
        <span class="tile-icon" aria-hidden="true">
          <Icon size={20} strokeWidth={1.75} />
        </span>
        <span class="tier-pill {tierClass(userTier)}">{tierDisplay(userTier)}</span>
      </div>
      <div class="tile-label">{tile.label}</div>
      <div class="tile-desc">{tile.description}</div>
    </button>
  {/each}
</div>

<style>
  .asset-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: var(--sp-3);
  }

  .tile {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-4);
    background: var(--surface-1);
    border: 1.5px solid var(--border);
    border-radius: var(--r-lg);
    cursor: pointer;
    text-align: left;
    transition:
      background var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard),
      transform var(--dur-fast) var(--ease-standard);
    outline: none;
  }

  .tile:hover:not(.selected) {
    background: var(--surface-2);
    border-color: var(--border-strong);
    transform: translateY(-1px);
  }

  .tile:focus-visible {
    box-shadow: var(--shadow-glow-accent);
  }

  .tile.selected {
    background: var(--accent-tint);
    border-color: var(--accent);
  }

  .tile-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
  }

  .tile-icon {
    display: inline-flex;
    color: var(--fg-muted);
    transition: color var(--dur-fast) var(--ease-standard);
  }

  .tile.selected .tile-icon {
    color: var(--accent-hi);
  }

  .tile-label {
    font-size: var(--text-heading);
    font-weight: var(--weight-semi);
    color: var(--fg);
    letter-spacing: -0.005em;
  }

  .tile-desc {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    line-height: 1.45;
  }

  /* Tier pills */
  .tier-pill {
    display: inline-flex;
    align-items: center;
    padding: 2px 7px;
    border-radius: var(--r-pill);
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    white-space: nowrap;
    border: 1px solid transparent;
    font-variant-numeric: tabular-nums;
  }

  .tier-unknown {
    background: var(--surface-2);
    color: var(--fg-subtle);
  }
  .tier-free {
    background: rgba(92, 101, 119, 0.15);
    color: var(--fg-muted);
  }
  .tier-value {
    background: rgba(56, 132, 255, 0.10);
    color: rgb(56, 132, 255);
    border-color: rgba(56, 132, 255, 0.20);
  }
  .tier-standard {
    background: rgba(34, 175, 109, 0.12);
    color: rgb(34, 175, 109);
    border-color: rgba(34, 175, 109, 0.22);
  }
  .tier-pro {
    background: rgba(244, 196, 48, 0.14);
    color: rgb(212, 158, 0);
    border-color: rgba(244, 196, 48, 0.30);
  }
</style>

<script lang="ts">
  /**
   * Step 5 — Granularity.
   * Only shown when the selected data kind supports interval selection
   * (OHLC, quote ticks). Hidden for pure trade ticks and EOD.
   */

  let {
    kindId,
    interval = $bindable("0"),
  }: {
    kindId: string;
    interval: string;
  } = $props();

  type IntervalOption = { id: string; label: string; description: string };
  const OPTIONS: IntervalOption[] = [
    { id: "0",   label: "Tick by tick", description: "Every event as it happened" },
    { id: "1s",  label: "1 second",     description: "1-second bars / samples" },
    { id: "60s", label: "1 minute",     description: "1-minute bars / samples" },
    { id: "300s",label: "5 minutes",    description: "5-minute bars / samples" },
    { id: "3600s",label: "1 hour",      description: "Hourly bars / samples" },
  ];

  // Whether granularity applies to this kind at all
  const isApplicable = $derived.by(() => {
    if (!kindId) return false;
    // Pure trade ticks — always tick, no interval selector
    if (kindId === "stock_trade" || kindId === "option_trade") return false;
    // EOD / OI / snapshot datasets — no interval concept
    if (kindId.includes("eod") || kindId.includes("oi") ||
        kindId === "index_ohlc" || kindId === "rate_levels" || kindId === "rate_dv01") return false;
    // Index levels always tick — no aggregation option yet
    if (kindId === "index_levels") return false;
    return true;
  });
</script>

{#if isApplicable}
  <div class="interval-picker" role="radiogroup" aria-label="Data granularity">
    {#each OPTIONS as opt}
      <button
        type="button"
        role="radio"
        aria-checked={interval === opt.id}
        class="interval-btn"
        class:active={interval === opt.id}
        onclick={() => (interval = opt.id)}
      >
        <span class="int-label">{opt.label}</span>
        <span class="int-desc">{opt.description}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .interval-picker {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }

  .interval-btn {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: var(--sp-2) var(--sp-3);
    background: var(--surface-1);
    border: 1.5px solid var(--border);
    border-radius: var(--r-md);
    cursor: pointer;
    text-align: left;
    min-width: 120px;
    transition:
      background var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
    outline: none;
  }

  .interval-btn:hover:not(.active) {
    background: var(--surface-2);
    border-color: var(--border-strong);
  }

  .interval-btn:focus-visible {
    box-shadow: var(--shadow-glow-accent);
  }

  .interval-btn.active {
    background: var(--accent-tint);
    border-color: var(--accent);
  }

  .int-label {
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    color: var(--fg);
    transition: color var(--dur-fast) var(--ease-standard);
  }

  .interval-btn.active .int-label {
    color: var(--accent-hi);
  }

  .int-desc {
    font-size: var(--text-caption);
    color: var(--fg-subtle);
    text-transform: none;
    letter-spacing: 0;
    font-weight: var(--weight-normal);
  }
</style>

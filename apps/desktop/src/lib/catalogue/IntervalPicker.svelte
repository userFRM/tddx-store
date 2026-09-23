<script lang="ts">
  /**
   * Step 5 — Granularity.
   * Only shown when the selected data kind supports interval selection
   * (OHLC, quote ticks). Hidden for pure trade ticks and EOD.
   */

  import { api, resolveKindToEndpoint, type IntervalOption } from "$lib/api";
  import { app, log } from "$lib/stores/app.svelte";

  let {
    kindId,
    interval = $bindable("tick"),
  }: {
    kindId: string;
    interval: string;
  } = $props();

  // The option list is the backend's, not ours: ThetaData accepts a
  // fixed set of interval spellings and rejects everything else, so a
  // list written here would eventually offer a value that fails at
  // download time.
  let options = $state<IntervalOption[]>([]);

  // Reload whenever the dataset changes: the accepted set is per
  // endpoint, so a list fetched once would offer `tick` on an OHLC
  // dataset that refuses it.
  $effect(() => {
    const kind = kindId;
    if (!kind) return;
    api
      .intervalOptions(kind)
      .then((v) => (options = v))
      .catch((e) => log("error", `Interval list unavailable: ${e}`));
  });

  // If the dataset dropped the currently-selected interval, fall back
  // to the finest one it does accept rather than sending a value the
  // server will reject.
  $effect(() => {
    if (options.length === 0) return;
    if (!options.some((o) => o.id === interval)) {
      interval = options[0].id;
    }
  });

  // Granularity applies exactly when the endpoint declares an
  // `interval` parameter. Reading that off the catalogue beats guessing
  // from the name: the old name-pattern test hid the picker for every
  // dataset whose id merely contained "eod" or "oi", and showed it for
  // endpoints that take no interval at all.
  const isApplicable = $derived.by(() => {
    if (!kindId) return false;
    const endpoint = resolveKindToEndpoint(kindId);
    const entry = app.catalogue.find((e) => e.name === endpoint);
    if (!entry) return false;
    return entry.params.some((p) => p.name === "interval");
  });
</script>

{#if isApplicable}
  <div class="interval-picker" role="radiogroup" aria-label="Data granularity">
    {#each options as opt (opt.id)}
      <button
        type="button"
        role="radio"
        aria-checked={interval === opt.id}
        class="interval-btn"
        class:active={interval === opt.id}
        onclick={() => (interval = opt.id)}
      >
        <span class="int-label">{opt.label}</span>
        <span class="int-id">{opt.id}</span>
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
    display: inline-flex;
    align-items: baseline;
    gap: var(--sp-2);
    padding: 6px var(--sp-3);
    background: var(--surface-1);
    border: 1.5px solid var(--border);
    border-radius: var(--r-md);
    cursor: pointer;
    text-align: left;
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

  .int-id {
    font-family: var(--font-mono);
    font-size: var(--text-caption);
    color: var(--fg-subtle);
    text-transform: none;
    letter-spacing: 0;
    font-weight: var(--weight-normal);
  }
</style>

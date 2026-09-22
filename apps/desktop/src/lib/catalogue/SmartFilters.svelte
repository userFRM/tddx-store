<script lang="ts">
  /**
   * SmartFilters — friendly shortcut controls for common option params.
   *
   * When the selected catalogue entry declares max_dte, strike_range,
   * expiration or right, presents guided shortcuts that write resolved
   * values into the shared `values` record. Every name written here is
   * a real registry parameter — anything else is dropped on the way to
   * the request, silently.
   *
   * Shortcuts shown:
   *   - Right: Both / Calls / Puts
   *   - DTE: quick buttons + slider  (max_dte)
   *   - Strikes: whole chain / ±n around spot  (strike_range)
   *   - Expiration: All / Next 4 monthlies / Specific date
   */
  import { ChevronDown, ChevronRight, Loader2, AlertCircle } from "lucide-svelte";
  import IconCall from "$lib/icons/IconCall.svelte";
  import IconPut from "$lib/icons/IconPut.svelte";
  import { api } from "$lib/api";
  import type { EndpointParam } from "$lib/api";

  let {
    params,
    symbol,
    values = $bindable<Record<string, string>>({}),
  }: {
    params: EndpointParam[];
    symbol: string;
    values: Record<string, string>;
  } = $props();

  // Which smart params does this endpoint expose?
  const paramNames = $derived(new Set(params.map((p) => p.name)));
  const has = $derived((name: string) => paramNames.has(name));

  const hasRight      = $derived(has("right"));
  const hasDte        = $derived(has("max_dte"));
  const hasStrikeRange = $derived(has("strike_range"));
  const hasStrike     = $derived(has("strike"));
  const hasExpiration = $derived(has("expiration"));

  const anySmartFilter = $derived(hasRight || hasDte || hasStrikeRange || hasStrike || hasExpiration);

  // ── Right ────────────────────────────────────────────────────
  function setRight(val: "both" | "C" | "P") {
    values = { ...values, right: val };
  }
  const rightVal = $derived((values["right"] ?? "both") as "both" | "C" | "P");
  const RIGHTS = [
    { id: "both" as const, label: "Both" },
    { id: "C" as const, label: "Calls" },
    { id: "P" as const, label: "Puts" },
  ];

  // ── DTE ─────────────────────────────────────────────────────
  const DTE_PRESETS = [
    { label: "0DTE",  max: 0 },
    { label: "7d",    max: 7 },
    { label: "30d",   max: 30 },
    { label: "90d",   max: 90 },
  ];

  let dteSlider = $state(365);

  function applyDte(max: number) {
    dteSlider = max;
    const update: Record<string, string> = { ...values };
    if (has("max_dte")) update["max_dte"] = String(max);
    values = update;
  }

  // ── Strike range (server-side, around spot) ────────────────────
  //
  // The endpoint declares one `strike_range: Int` parameter, documented
  // as: "for a specified value n, returns n strikes above and n below
  // the spot price, plus the ATM strike — a maximum of 2n+1 strikes."
  //
  // So the bracket is resolved server-side against the spot on each
  // requested date. An earlier version of this panel tried to do it in
  // the browser — fetch a quote, compute a dollar band, snap it to the
  // strike list — writing `strike_filter_low`/`strike_filter_high`,
  // which are not parameters any endpoint has. Nothing it produced ever
  // reached a request.
  const STRIKE_RANGE_PRESETS = [
    { n: 1, label: "ATM ±1" },
    { n: 5, label: "±5" },
    { n: 10, label: "±10" },
    { n: 25, label: "±25" },
  ];

  const strikeRange = $derived(values["strike_range"] ?? "");

  function applyStrikeRange(n: number | null) {
    const update = { ...values };
    if (n === null) delete update["strike_range"];
    else update["strike_range"] = String(n);
    values = update;
  }

  // ── Expiration ────────────────────────────────────────────────
  let expMode   = $state<"all" | "monthlies" | "specific">("all");
  let expDate   = $state("");
  let expLoading = $state(false);
  let expError  = $state("");

  /** True iff a date falls on the 3rd Friday of its month */
  function isThirdFriday(d: Date): boolean {
    if (d.getDay() !== 5) return false; // not Friday
    const day = d.getDate();
    return day >= 15 && day <= 21;
  }

  async function applyMonthlies() {
    if (!symbol) { expError = "Select a symbol first."; return; }
    expLoading = true;
    expError = "";
    try {
      const rows = await api.listQuery({
        endpoint: "option_list_expirations",
        args: { symbol },
      });
      const today = new Date();
      const monthlies = rows
        .map((r) => {
          const s = r.trim();
          if (s.length === 8) {
            return new Date(`${s.slice(0,4)}-${s.slice(4,6)}-${s.slice(6,8)}`);
          }
          return new Date(s);
        })
        .filter((d) => !isNaN(d.getTime()) && d > today && isThirdFriday(d))
        .sort((a, b) => a.getTime() - b.getTime())
        .slice(0, 4)
        .map((d) => {
          const y = d.getFullYear();
          const m = String(d.getMonth() + 1).padStart(2, "0");
          const day = String(d.getDate()).padStart(2, "0");
          return `${y}${m}${day}`;
        });

      if (monthlies.length === 0) {
        expError = "No upcoming monthly expirations found.";
      } else {
        values = { ...values, expiration: monthlies.join(",") };
      }
    } catch (e) {
      expError = e instanceof Error ? e.message : String(e);
    } finally {
      expLoading = false;
    }
  }

  function applyExpMode(mode: "all" | "monthlies" | "specific") {
    expMode = mode;
    expError = "";
    if (mode === "all") {
      values = { ...values, expiration: "*" };
    } else if (mode === "monthlies") {
      void applyMonthlies();
    }
    // "specific" waits for expDate input
  }

  $effect(() => {
    if (expMode !== "specific" || !expDate) return;
    const want = expDate.replace(/-/g, "");
    if (values["expiration"] === want) return;
    values = { ...values, expiration: want };
  });

  // Note: ownedParams is NOT exported from this component. The caller (BrowseView)
  // maintains its own SMART_FILTER_PARAMS set and passes it as excludeNames to ParamForm.
</script>

{#if anySmartFilter}
  <div class="smart-filters">
    <div class="sf-header">
      <span class="sf-title text-caption">Quick filters</span>
    </div>

    <div class="sf-body">
      <!-- ── Right ── -->
      {#if hasRight}
        <div class="sf-row">
          <span class="sf-label">Right</span>
          <div class="tile-picker" role="radiogroup" aria-label="Option right">
            {#each RIGHTS as opt (opt.id)}
              <button
                type="button"
                role="radio"
                aria-checked={rightVal === opt.id}
                class="tile-btn"
                class:active={rightVal === opt.id}
                onclick={() => setRight(opt.id)}
              >
                {#if opt.id === "C"}<IconCall size={13} />{/if}
                {#if opt.id === "P"}<IconPut size={13} />{/if}
                {opt.label}
              </button>
            {/each}
          </div>
        </div>
      {/if}

      <!-- ── DTE ── -->
      {#if hasDte}
        <div class="sf-row">
          <span class="sf-label">DTE</span>
          <div class="dte-controls">
            <div class="dte-presets">
              {#each DTE_PRESETS as p (p.label)}
                <button
                  type="button"
                  class="tile-btn"
                  class:active={Number(values["max_dte"]) === p.max}
                  onclick={() => applyDte(p.max)}
                >{p.label}</button>
              {/each}
            </div>
            <div class="dte-slider-row">
              <input
                type="range"
                min="0"
                max="1825"
                step="1"
                class="dte-slider"
                bind:value={dteSlider}
                oninput={() => applyDte(dteSlider)}
                aria-label="Max DTE"
              />
              <span class="dte-val tabnum">{dteSlider}d</span>
            </div>
          </div>
        </div>
      {/if}

      <!-- ── Strike range ── -->
      {#if hasStrikeRange}
        <div class="sf-row">
          <span class="sf-label">Strikes</span>
          <div class="dte-controls">
            <div class="dte-presets">
              <button
                type="button"
                class="tile-btn"
                class:active={strikeRange === ""}
                onclick={() => applyStrikeRange(null)}
              >Whole chain</button>
              {#each STRIKE_RANGE_PRESETS as p (p.n)}
                <button
                  type="button"
                  class="tile-btn"
                  class:active={strikeRange === String(p.n)}
                  onclick={() => applyStrikeRange(p.n)}
                >{p.label}</button>
              {/each}
            </div>
            <p class="sf-hint text-caption">
              {#if strikeRange}
                Up to {2 * Number(strikeRange) + 1} strikes around the spot price on each date.
              {:else}
                Every strike listed on each date.
              {/if}
            </p>
          </div>
        </div>
      {/if}

      <!-- ── Expiration ── -->
      {#if hasExpiration}
        <div class="sf-row">
          <span class="sf-label">Expiration</span>
          <div class="exp-controls">
            <div class="tile-picker" role="radiogroup" aria-label="Expiration filter">
              {#each [
                { id: "all"        as const, label: "All" },
                { id: "monthlies" as const, label: "Next 4 monthlies" },
                { id: "specific"  as const, label: "Specific date" },
              ] as opt (opt.id)}
                <button
                  type="button"
                  role="radio"
                  aria-checked={expMode === opt.id}
                  class="tile-btn"
                  class:active={expMode === opt.id}
                  onclick={() => applyExpMode(opt.id)}
                >{opt.label}</button>
              {/each}
            </div>

            {#if expLoading}
              <div class="exp-loading">
                <Loader2 size={13} strokeWidth={1.75} class="spin" />
                <span class="text-body-sm fg-muted">Looking up expirations…</span>
              </div>
            {/if}

            {#if expError}
              <div class="atm-error">
                <AlertCircle size={13} strokeWidth={1.75} />
                <span>{expError}</span>
              </div>
            {/if}

            {#if expMode === "specific"}
              <input
                type="date"
                class="field-input exp-date-input"
                bind:value={expDate}
                aria-label="Specific expiration date"
              />
            {/if}

            {#if expMode === "monthlies" && values["expiration"] && values["expiration"] !== "*"}
              <p class="exp-resolved text-caption">
                Resolved: <span class="tabnum">{values["expiration"]}</span>
              </p>
            {/if}
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .smart-filters {
    display: flex;
    flex-direction: column;
    gap: 0;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    overflow: hidden;
  }

  .sf-header {
    padding: var(--sp-2) var(--sp-4);
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
  }

  .sf-title {
    color: var(--fg-subtle);
  }

  .sf-body {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .sf-row {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-4);
    padding: var(--sp-4);
    border-bottom: 1px solid var(--border);
  }

  .sf-row:last-child {
    border-bottom: none;
  }


  .sf-label {
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    color: var(--fg-muted);
    min-width: 80px;
    padding-top: 6px;
    flex-shrink: 0;
  }

  /* Shared tile picker */
  .tile-picker {
    display: flex;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }

  .tile-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: var(--sp-2) var(--sp-4);
    background: var(--surface-2);
    border: 1.5px solid var(--border);
    border-radius: var(--r-sm);
    color: var(--fg-muted);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
    outline: none;
  }

  .tile-btn:hover:not(.active) {
    background: var(--surface-3);
    border-color: var(--border-strong);
    color: var(--fg);
  }

  .tile-btn.active {
    background: var(--accent-tint);
    border-color: var(--accent);
    color: var(--accent-hi);
  }

  .tile-btn:focus-visible { box-shadow: var(--shadow-glow-accent); }

  /* DTE */
  .dte-controls {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    flex: 1;
  }

  .dte-presets {
    display: flex;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }

  .dte-slider-row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  .dte-slider {
    flex: 1;
    accent-color: var(--accent);
    height: 4px;
    cursor: pointer;
    max-width: 280px;
  }

  .sf-hint { margin: 2px 0 0; color: var(--fg-subtle); }
  .dte-val {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
    min-width: 36px;
  }

  /* Strike range */

  .field-input {
    height: 32px;
    padding: 0 var(--sp-3);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    color: var(--fg);
    font-size: var(--text-body-sm);
    font-family: var(--font-ui);
    outline: none;
    transition:
      border-color var(--dur-fast) var(--ease-standard),
      box-shadow var(--dur-fast) var(--ease-standard);
  }

  .field-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-tint);
  }

  .field-input::placeholder { color: var(--fg-subtle); }



  /* ATM panel */




  .atm-error {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--text-body-sm);
    color: var(--bad);
  }




  :global(.btn-resolve .spin) {
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  /* Expiration */
  .exp-controls {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    flex: 1;
  }

  .exp-loading {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }

  :global(.exp-loading .spin) {
    animation: spin 0.7s linear infinite;
    color: var(--fg-subtle);
  }

  .exp-date-input {
    width: 160px;
  }

  .exp-resolved {
    color: var(--fg-subtle);
    font-weight: var(--weight-normal);
    text-transform: none;
    letter-spacing: 0;
    margin: 0;
    word-break: break-all;
  }
</style>

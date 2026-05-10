<script lang="ts">
  /**
   * SmartFilters — friendly shortcut controls for common option params.
   *
   * When the selected catalogue entry has params named max_dte, min_dte,
   * strike_filter_low, strike_filter_high, expiration, strike, or right,
   * presents guided shortcuts that write resolved values into the shared
   * `values` record.
   *
   * Shortcuts shown:
   *   - Right: Both / Calls / Puts
   *   - DTE: quick buttons + slider
   *   - Strike — Range: low/high dollar inputs
   *   - Strike — ATM ±N%: helper panel that resolves spot + strikes live
   *   - Expiration: All / Next 4 monthlies / Specific date
   */
  import { ChevronDown, ChevronRight, Loader2, AlertCircle } from "lucide-svelte";
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
  const hasDte        = $derived(has("max_dte") || has("min_dte"));
  const hasStrikeRange = $derived(has("strike_filter_low") && has("strike_filter_high"));
  const hasStrike     = $derived(has("strike"));
  const hasExpiration = $derived(has("expiration"));

  const anySmartFilter = $derived(hasRight || hasDte || hasStrikeRange || hasStrike || hasExpiration);

  // ── Right ────────────────────────────────────────────────────
  function setRight(val: "both" | "C" | "P") {
    values = { ...values, right: val };
  }
  const rightVal = $derived((values["right"] ?? "both") as "both" | "C" | "P");

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

  // ── Strike Range ──────────────────────────────────────────────
  let strikeLow  = $state(values["strike_filter_low"]  ?? "");
  let strikeHigh = $state(values["strike_filter_high"] ?? "");

  // Mirror local strike inputs into `values` only when they actually
  // differ. Writing every fire (even no-op) re-publishes a fresh
  // object reference, which retriggers parent reactivity, retriggers
  // this effect → infinite loop.
  $effect(() => {
    const wantLow  = has("strike_filter_low")  ? strikeLow  : "";
    const wantHigh = has("strike_filter_high") ? strikeHigh : "";
    if (!wantLow && !wantHigh) return;
    const currLow  = values["strike_filter_low"]  ?? "";
    const currHigh = values["strike_filter_high"] ?? "";
    if (wantLow === currLow && wantHigh === currHigh) return;
    const update: Record<string, string> = { ...values };
    if (has("strike_filter_low")  && strikeLow)  update["strike_filter_low"]  = strikeLow;
    if (has("strike_filter_high") && strikeHigh) update["strike_filter_high"] = strikeHigh;
    values = update;
  });

  // ── ATM ±N% Helper ────────────────────────────────────────────
  let atmOpen    = $state(false);
  let atmPct     = $state(10);
  let atmLoading = $state(false);
  let atmError   = $state("");

  async function resolveAtm() {
    if (!symbol) { atmError = "Select a symbol first."; return; }
    atmLoading = true;
    atmError = "";
    try {
      // Fetch spot price via stock_snapshot_quote
      const quoteRows = await api.listQuery({
        endpoint: "stock_snapshot_quote",
        args: { root: symbol },
      });
      if (!quoteRows || quoteRows.length === 0) throw new Error("No quote data returned.");
      // Expect a numeric string (the ask or last price)
      const spot = parseFloat(quoteRows[0]);
      if (isNaN(spot) || spot <= 0) throw new Error(`Could not parse spot price: ${quoteRows[0]}`);

      const factor = atmPct / 100;
      const low  = (spot * (1 - factor)).toFixed(2);
      const high = (spot * (1 + factor)).toFixed(2);

      strikeLow  = low;
      strikeHigh = high;

      // Also fetch available strikes and snap to nearest bracket if possible
      const expParam = values["expiration"] && values["expiration"] !== "*"
        ? values["expiration"]
        : "*";
      const strikesRaw = await api.listQuery({
        endpoint: "option_list_strikes",
        args: { root: symbol, expiration: expParam },
      }).catch(() => [] as string[]);

      if (strikesRaw.length > 0) {
        const strikes = strikesRaw.map(Number).filter((n) => !isNaN(n)).sort((a, b) => a - b);
        const lo = Math.min(...strikes.filter((s) => s >= parseFloat(low)));
        const hi = Math.max(...strikes.filter((s) => s <= parseFloat(high)));
        if (isFinite(lo)) strikeLow  = String(lo);
        if (isFinite(hi)) strikeHigh = String(hi);
      }

      atmOpen = false;
    } catch (e) {
      atmError = e instanceof Error ? e.message : String(e);
    } finally {
      atmLoading = false;
    }
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
        args: { root: symbol },
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
            {#each [
              { id: "both" as const, label: "Both" },
              { id: "C"    as const, label: "Calls" },
              { id: "P"    as const, label: "Puts" },
            ] as opt (opt.id)}
              <button
                type="button"
                role="radio"
                aria-checked={rightVal === opt.id}
                class="tile-btn"
                class:active={rightVal === opt.id}
                onclick={() => setRight(opt.id)}
              >{opt.label}</button>
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

      <!-- ── Strike Range ── -->
      {#if hasStrikeRange || hasStrike}
        <div class="sf-row">
          <span class="sf-label">Strike range</span>
          <div class="strike-range-row">
            <input
              type="number"
              class="field-input strike-input"
              bind:value={strikeLow}
              placeholder="Low ($)"
              aria-label="Strike range low"
              min="0"
              step="0.5"
            />
            <span class="range-sep">–</span>
            <input
              type="number"
              class="field-input strike-input"
              bind:value={strikeHigh}
              placeholder="High ($)"
              aria-label="Strike range high"
              min="0"
              step="0.5"
            />
          </div>
        </div>
      {/if}

      <!-- ── ATM ±N% ── -->
      {#if hasStrikeRange || hasStrike}
        <div class="sf-row sf-row-indent">
          <button
            type="button"
            class="atm-toggle"
            onclick={() => { atmOpen = !atmOpen; atmError = ""; }}
            aria-expanded={atmOpen}
          >
            {#if atmOpen}
              <ChevronDown size={12} strokeWidth={1.75} />
            {:else}
              <ChevronRight size={12} strokeWidth={1.75} />
            {/if}
            ATM ±N% shortcut
          </button>

          {#if atmOpen}
            <div class="atm-panel">
              <label class="atm-label">
                Bracket: ±<span class="tabnum">{atmPct}</span>%
                <input
                  type="range"
                  min="5"
                  max="50"
                  step="1"
                  class="dte-slider"
                  bind:value={atmPct}
                  aria-label="ATM bracket percentage"
                />
              </label>

              {#if atmError}
                <div class="atm-error">
                  <AlertCircle size={13} strokeWidth={1.75} />
                  <span>{atmError}</span>
                </div>
              {/if}

              <button
                type="button"
                class="btn-resolve"
                onclick={resolveAtm}
                disabled={atmLoading || !symbol}
              >
                {#if atmLoading}
                  <Loader2 size={13} strokeWidth={1.75} class="spin" />
                  Resolving…
                {:else}
                  Resolve strikes
                {/if}
              </button>
              {#if !symbol}
                <p class="atm-hint">Select a symbol in step 2 first.</p>
              {/if}
            </div>
          {/if}
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

  .sf-row-indent {
    padding-left: calc(var(--sp-4) + var(--sp-8));
    background: var(--surface-1);
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

  .dte-val {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
    min-width: 36px;
  }

  /* Strike range */
  .strike-range-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }

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

  .strike-input {
    width: 110px;
  }

  .range-sep {
    font-size: var(--text-body-sm);
    color: var(--fg-subtle);
    flex-shrink: 0;
  }

  /* ATM panel */
  .atm-toggle {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    background: transparent;
    border: none;
    color: var(--fg-muted);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    cursor: pointer;
    padding: 0;
    transition: color var(--dur-fast) var(--ease-standard);
    outline: none;
  }

  .atm-toggle:hover { color: var(--fg); }
  .atm-toggle:focus-visible { box-shadow: var(--shadow-glow-accent); border-radius: var(--r-sm); }

  .atm-panel {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-4);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    margin-top: var(--sp-2);
    max-width: 360px;
  }

  .atm-label {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .atm-error {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--text-body-sm);
    color: var(--bad);
  }

  .atm-hint {
    font-size: var(--text-caption);
    color: var(--fg-subtle);
    text-transform: none;
    letter-spacing: 0;
    font-weight: var(--weight-normal);
    margin: 0;
  }

  .btn-resolve {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    height: 30px;
    padding: 0 var(--sp-4);
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: var(--r-sm);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    cursor: pointer;
    transition: filter var(--dur-fast) var(--ease-standard);
    align-self: flex-start;
  }

  .btn-resolve:hover:not(:disabled) { filter: brightness(1.08); }
  .btn-resolve:disabled { opacity: 0.5; cursor: not-allowed; }

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

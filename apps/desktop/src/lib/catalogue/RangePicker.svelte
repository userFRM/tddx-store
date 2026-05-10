<script lang="ts">
  /**
   * Step 4 — What time range?
   * Quick pills (1Y / 2Y / 3Y / 5Y / 10Y / Max) + Custom toggle for
   * explicit from/to date inputs. Also shows an estimated row/size
   * estimate when symbol count and a quick-pill are selected.
   */

  let {
    start = $bindable(""),
    end = $bindable(""),
    symbolCount = 0,
    kindId = "",
  }: {
    start: string;
    end: string;
    symbolCount: number;
    kindId: string;
  } = $props();

  type QuickPill = { id: string; label: string; years: number | null };
  const PILLS: QuickPill[] = [
    { id: "1y",  label: "1Y",  years: 1 },
    { id: "2y",  label: "2Y",  years: 2 },
    { id: "3y",  label: "3Y",  years: 3 },
    { id: "5y",  label: "5Y",  years: 5 },
    { id: "10y", label: "10Y", years: 10 },
    { id: "max", label: "Max", years: null },
  ];

  let activePill = $state<string>("3y");

  // Rough trading-day estimates: ~252 days/year
  const TRADING_DAYS_PER_YEAR = 252;
  // Avg rows per day (kind-specific approximation)
  function avgRowsPerDay(kind: string): number {
    if (kind.includes("trade_quote")) return 180_000;
    if (kind.includes("trade"))       return 90_000;
    if (kind.includes("quote"))       return 600_000;
    if (kind.includes("ohlc") || kind.includes("eod")) return 1;
    if (kind.includes("oi"))          return 50;
    return 50_000;
  }
  // Avg bytes per row (parquet compressed)
  function avgBytesPerRow(kind: string): number {
    if (kind.includes("trade_quote")) return 48;
    if (kind.includes("trade"))       return 32;
    if (kind.includes("quote"))       return 28;
    if (kind.includes("ohlc") || kind.includes("eod")) return 20;
    return 32;
  }

  function fmtBytes(n: number): string {
    const u = ["B", "KB", "MB", "GB", "TB"];
    let i = 0; let v = n;
    while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
    return `~${v.toFixed(i === 0 ? 0 : 1)} ${u[i]}`;
  }

  function fmtRows(n: number): string {
    if (n >= 1_000_000_000) return `~${(n / 1_000_000_000).toFixed(1)}B rows`;
    if (n >= 1_000_000)     return `~${(n / 1_000_000).toFixed(1)}M rows`;
    if (n >= 1_000)         return `~${(n / 1_000).toFixed(0)}K rows`;
    return `~${n} rows`;
  }

  function todayStr(): string {
    return new Date().toISOString().slice(0, 10);
  }

  function yearsAgoStr(years: number): string {
    const d = new Date();
    d.setFullYear(d.getFullYear() - years);
    return d.toISOString().slice(0, 10);
  }

  function selectPill(pill: QuickPill) {
    activePill = pill.id;
    if (pill.years === null) {
      // "Max" — back to ~2010-01-04 (roughly ThetaData earliest data)
      start = "2010-01-04";
      end = todayStr();
    } else {
      start = yearsAgoStr(pill.years);
      end = todayStr();
    }
  }

  // When the user edits start/end directly, deactivate the matching
  // pill so the active state always reflects ground truth — pills are
  // shortcuts, the date inputs are the source of truth.
  function onDateInput() {
    activePill = "";
  }

  // Initialize default on mount
  $effect.pre(() => {
    const defaultPill = PILLS.find((p) => p.id === "3y");
    if (defaultPill && !start && !end) {
      selectPill(defaultPill);
    }
  });

  // Estimate
  const estimate = $derived.by(() => {
    if (!start || !end || symbolCount === 0) return null;
    const startDate = new Date(start);
    const endDate   = new Date(end);
    if (isNaN(startDate.getTime()) || isNaN(endDate.getTime())) return null;
    const calDays = Math.max(0, (endDate.getTime() - startDate.getTime()) / 86_400_000);
    const tradingDays = Math.round(calDays * TRADING_DAYS_PER_YEAR / 365);
    const rPd = avgRowsPerDay(kindId);
    const bPr = avgBytesPerRow(kindId);
    const totalRows  = symbolCount * tradingDays * rPd;
    const totalBytes = totalRows * bPr;
    return { tradingDays, totalRows, totalBytes };
  });
</script>

<div class="range-picker">
  <!-- Date inputs (always visible — single source of truth) -->
  <div class="custom-dates" role="group" aria-label="Date range">
    <label class="date-label">
      <span class="date-caption">From</span>
      <input
        type="date"
        class="field-input date-input"
        bind:value={start}
        oninput={onDateInput}
        max={end || todayStr()}
      />
    </label>
    <span class="date-sep" aria-hidden="true">—</span>
    <label class="date-label">
      <span class="date-caption">To</span>
      <input
        type="date"
        class="field-input date-input"
        bind:value={end}
        oninput={onDateInput}
        min={start}
        max={todayStr()}
      />
    </label>
  </div>

  <!-- Quick pills as shortcuts that pre-fill the inputs above -->
  <div class="pill-row" role="group" aria-label="Quick date range presets">
    <span class="text-caption pill-row-hint">Quick:</span>
    {#each PILLS as pill}
      <button
        type="button"
        class="quick-pill"
        class:active={activePill === pill.id}
        onclick={() => selectPill(pill)}
        aria-pressed={activePill === pill.id}
      >
        {pill.label}
      </button>
    {/each}
  </div>

  <!-- Estimate row -->
  {#if estimate}
    <div class="estimate" aria-label="Size estimate">
      <span class="estimate-line">
        <span class="tabnum">{symbolCount.toLocaleString()}</span> symbols
        &times;
        <span class="tabnum">{estimate.tradingDays.toLocaleString()}</span> trading days
        = <span class="estimate-val tabnum">{fmtRows(estimate.totalRows)}</span>,
        <span class="estimate-val tabnum">{fmtBytes(estimate.totalBytes)}</span>
        <span class="estimate-note">(Parquet compressed estimate)</span>
      </span>
    </div>
  {/if}
</div>

<style>
  .range-picker {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .pill-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .pill-row-hint {
    color: var(--fg-subtle);
    margin-right: var(--sp-1);
  }

  .quick-pill {
    padding: 5px 14px;
    height: 32px;
    border-radius: var(--r-pill);
    background: var(--surface-1);
    border: 1.5px solid var(--border);
    color: var(--fg-muted);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    cursor: pointer;
    font-variant-numeric: tabular-nums;
    transition:
      background var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
    outline: none;
  }

  .quick-pill:hover:not(.active) {
    background: var(--surface-2);
    border-color: var(--border-strong);
    color: var(--fg);
  }

  .quick-pill:focus-visible {
    box-shadow: var(--shadow-glow-accent);
  }

  .quick-pill.active {
    background: var(--accent-tint);
    border-color: var(--accent);
    color: var(--accent-hi);
    font-weight: var(--weight-semi);
  }

  .custom-dates {
    display: flex;
    align-items: flex-end;
    gap: var(--sp-3);
  }

  .date-label {
    display: flex;
    flex-direction: column;
    gap: 5px;
    flex: 1;
  }

  .date-caption {
    font-size: var(--text-caption);
    color: var(--fg-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-weight: var(--weight-medium);
  }

  .date-input {
    height: 32px;
    padding: 0 var(--sp-3);
    font-family: var(--font-mono);
    font-size: var(--text-body-sm);
    font-variant-numeric: tabular-nums;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    color: var(--fg);
    outline: none;
    transition: border-color var(--dur-fast) var(--ease-standard),
                box-shadow var(--dur-fast) var(--ease-standard);
    width: 100%;
  }

  .date-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-tint);
  }

  .date-sep {
    color: var(--fg-subtle);
    padding-bottom: 6px;
    flex-shrink: 0;
  }

  .estimate {
    padding: var(--sp-2) var(--sp-3);
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
  }

  .estimate-line {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    line-height: 1.5;
  }

  .estimate-val {
    color: var(--fg);
    font-weight: var(--weight-medium);
  }

  .estimate-note {
    color: var(--fg-subtle);
    font-size: var(--text-caption);
    text-transform: none;
    letter-spacing: 0;
    font-weight: var(--weight-normal);
  }
</style>

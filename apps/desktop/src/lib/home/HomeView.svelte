<script lang="ts">
  /**
   * HomeView — default landing view.
   *
   * Sections (vertical scroll):
   *   1. Greeting + connection state
   *   2. Subscription summary card (4 asset-class tier pills, enlarged)
   *   3. Library snapshot (disk stats + top-5 recent coverage entries)
   *   4. Quick actions (Browse / Library / Queue)
   *   5. Recently queued (up to 8 compact rows from queueSnap.recent)
   */

  import {
    Compass,
    Library,
    ListChecks,
    ArrowUpRight,
    ShieldCheck,
    ShieldAlert,
    HardDrive,
    FileStack,
    Database,
    Clock,
  } from "lucide-svelte";
  import { onMount } from "svelte";
  import { app, navigate } from "$lib/stores/app.svelte";
  import { api, fmtBytes, type Coverage, type TierName } from "$lib/api";
  import { openUrl } from "@tauri-apps/plugin-opener";

  // ── Library snapshot ─────────────────────────────────────────
  let coverage = $state<Coverage[]>([]);
  let coverageLoading = $state(false);

  onMount(async () => {
    if (app.connState !== "connected") return;
    coverageLoading = true;
    try {
      coverage = await api.coverage();
    } catch {
      // not yet connected — silently skip
    } finally {
      coverageLoading = false;
    }
  });

  // Re-fetch when connection is established mid-session.
  $effect(() => {
    if (app.connState === "connected" && coverage.length === 0 && !coverageLoading) {
      coverageLoading = true;
      api.coverage()
        .then((r) => { coverage = r; })
        .catch(() => {})
        .finally(() => { coverageLoading = false; });
    }
  });

  // ── Derived stats ─────────────────────────────────────────────
  const snap = $derived(app.queueSnap);

  const totalFiles = $derived(snap?.files_on_disk ?? 0);
  const totalBytes = $derived(snap?.bytes_on_disk ?? 0);
  const totalDatasets = $derived(
    (() => {
      const seen = new Set<string>();
      for (const row of coverage) seen.add(`${row.symbol}:${row.kind}`);
      return seen.size;
    })()
  );

  // Top 5 most recent coverage entries (sorted by last date descending).
  const recentCoverage = $derived(
    [...coverage]
      .sort((a, b) => {
        const al = a.last ?? "";
        const bl = b.last ?? "";
        return bl.localeCompare(al);
      })
      .slice(0, 5)
  );

  // Recently queued: last 8 from the snapshot.
  const recentTasks = $derived(snap?.recent?.slice(0, 8) ?? []);

  // ── Greeting ──────────────────────────────────────────────────
  const greeting = $derived(
    app.connState === "connected" ? "Welcome back." : "Connect to ThetaData."
  );
  const greetingSub = $derived(
    app.connState === "connected"
      ? "Your data library and download queue are ready."
      : "Enter your ThetaData credentials in Settings to start downloading market data."
  );

  // ── Tier helpers ──────────────────────────────────────────────
  // Class list, labels, tier values, upgrade gating all come from
  // `tierStatus.classes` — built server-side from
  // `tdds_core::tier::AssetClass` + `Tier::workers()`. The FE does
  // zero math: rendering this view stays correct when the Rust side
  // gains a new asset class or changes the workers formula.
  const status = $derived(app.tierStatus);
  const showUpgrade = $derived(!!status && status.classes.some((c) => !c.at_max));

  function tierClass(tier: TierName): string {
    if (tier === "Pro") return "tier-pill tier-pro";
    if (tier === "Standard") return "tier-pill tier-standard";
    if (tier === "Value") return "tier-pill tier-value";
    return "tier-pill tier-free";
  }

  async function handleUpgrade() {
    const url = status?.upgrade_url ?? "https://thetadata.net/pricing";
    try { await openUrl(url); } catch {}
  }

  // ── Kind display label ────────────────────────────────────────
  function kindLabel(kind: string): string {
    const map: Record<string, string> = {
      stock_trade:       "Trade Tick",
      stock_quote:       "NBBO Quote",
      stock_trade_quote: "Trade-Quote",
      stock_history_trade: "Stock Trade",
      stock_history_quote: "Stock Quote",
      stock_history_eod:   "Stock EOD",
      stock_history_ohlc:  "Stock OHLC",
      option_trade:                       "Option Trade",
      option_quote:                       "Option Quote",
      option_trade_quote:                 "Option Trade-Quote",
      option_oi:                          "Open Interest",
      option_history_trade:               "Option Trade",
      option_history_quote:               "Option Quote",
      option_history_open_interest:       "Open Interest",
      option_history_greeks_implied_volatility: "Implied Vol",
      option_history_greeks_first_order:  "Greeks 1st",
      option_history_greeks_second_order: "Greeks 2nd",
      option_history_greeks_third_order:  "Greeks 3rd",
      index_ohlc:   "Index OHLC",
      index_levels: "Index Levels",
      index_history_eod:  "Index EOD",
      index_history_ohlc: "Index OHLC",
      rate_levels: "Rate Levels",
      rate_dv01:   "Rate DV01",
    };
    return map[kind] ?? kind;
  }

  // ── Status pill class ─────────────────────────────────────────
  function statusPill(status: string): string {
    if (status === "done")    return "pill pill-done";
    if (status === "running") return "pill pill-running";
    if (status === "failed")  return "pill pill-failed";
    if (status === "empty")   return "pill pill-empty";
    return "pill pill-pending";
  }
</script>

<div class="home-page">
  <div class="home-inner">

    <!-- ── Section 1: Greeting ──────────────────────────────── -->
    <section class="greeting-section" aria-label="Connection status and greeting">
      <div class="greeting-body">
        <h1 class="greeting-title">{greeting}</h1>
        <p class="greeting-sub">{greetingSub}</p>
      </div>
      {#if app.connState !== "connected"}
        <button
          type="button"
          class="btn btn-primary cta-connect"
          onclick={() => navigate("settings")}
        >
          Open Settings
          <ArrowUpRight size={15} strokeWidth={1.75} />
        </button>
      {/if}
    </section>

    <!-- ── Section 2: Subscription summary ─────────────────── -->
    {#if status}
      <section class="sub-card" aria-label="Subscription summary">
        <div class="sub-card-header">
          <span class="sub-icon" aria-hidden="true">
            {#if showUpgrade}
              <ShieldAlert size={16} strokeWidth={1.75} />
            {:else}
              <ShieldCheck size={16} strokeWidth={1.75} />
            {/if}
          </span>
          <span class="sub-title">Subscription</span>
          {#if showUpgrade}
            <button
              type="button"
              class="upgrade-btn-inline"
              onclick={handleUpgrade}
              aria-label="Open ThetaData pricing page"
            >
              Upgrade
              <ArrowUpRight size={12} strokeWidth={2} />
            </button>
          {/if}
        </div>

        <div class="tier-pills-row">
          {#each status.classes as c (c.class)}
            <div class={tierClass(c.tier)}>
              <span class="tier-pill-label text-caption">{c.label}</span>
              <span class="tier-pill-value tabnum">{c.tier}</span>
              {#if !c.at_max}
                <button
                  type="button"
                  class="tier-upgrade-mini"
                  onclick={handleUpgrade}
                  aria-label={`Upgrade ${c.label.toLowerCase()} tier`}
                >
                  <ArrowUpRight size={10} strokeWidth={2} />
                  Upgrade
                </button>
              {/if}
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <!-- ── Section 3: Library snapshot ─────────────────────── -->
    <section class="snapshot-section" aria-label="Library snapshot">
      <div class="section-header">
        <h2 class="section-title">Library</h2>
        <button
          type="button"
          class="btn btn-ghost section-link"
          onclick={() => navigate("library")}
        >
          View all
          <ArrowUpRight size={13} strokeWidth={1.75} />
        </button>
      </div>

      <div class="disk-stats">
        <div class="disk-stat">
          <Database size={14} strokeWidth={1.75} class="stat-icon" />
          <span class="stat-value tabnum">{totalDatasets.toLocaleString()}</span>
          <span class="stat-label text-caption">datasets</span>
        </div>
        <div class="disk-stat-divider" aria-hidden="true"></div>
        <div class="disk-stat">
          <FileStack size={14} strokeWidth={1.75} class="stat-icon" />
          <span class="stat-value tabnum">{totalFiles.toLocaleString()}</span>
          <span class="stat-label text-caption">files</span>
        </div>
        <div class="disk-stat-divider" aria-hidden="true"></div>
        <div class="disk-stat">
          <HardDrive size={14} strokeWidth={1.75} class="stat-icon" />
          <span class="stat-value tabnum">{fmtBytes(totalBytes)}</span>
          <span class="stat-label text-caption">on disk</span>
        </div>
      </div>

      {#if recentCoverage.length > 0}
        <div class="coverage-list" aria-label="Recent datasets">
          {#each recentCoverage as row (row.symbol + ":" + row.kind)}
            <button
              type="button"
              class="coverage-row"
              onclick={() => navigate("library")}
              aria-label="View {row.symbol} {kindLabel(row.kind)} in library"
            >
              <span class="cov-symbol tabnum">{row.symbol}</span>
              <span class="cov-kind">{kindLabel(row.kind)}</span>
              <span class="cov-meta tabnum">
                {#if row.first && row.last}
                  {row.first.slice(0, 4)}–{row.last.slice(0, 4)}
                {:else}
                  —
                {/if}
              </span>
              <span class="cov-size tabnum">{fmtBytes(row.bytes)}</span>
            </button>
          {/each}
        </div>
      {:else if !coverageLoading && app.connState === "connected"}
        <p class="empty-hint">No datasets downloaded yet. Use Browse to queue your first download.</p>
      {:else if app.connState !== "connected"}
        <p class="empty-hint">Connect to ThetaData to see your library.</p>
      {/if}
    </section>

    <!-- ── Section 4: Quick actions ─────────────────────────── -->
    <section class="quick-actions" aria-label="Quick actions">
      <h2 class="section-title">Quick actions</h2>
      <div class="action-row">
        <button
          type="button"
          class="action-card"
          onclick={() => navigate("browse")}
          aria-label="Browse datasets"
        >
          <Compass size={22} strokeWidth={1.75} class="action-icon" />
          <span class="action-label">Browse datasets</span>
          <span class="action-sub">Choose symbols and data types, then queue</span>
        </button>

        <button
          type="button"
          class="action-card"
          onclick={() => navigate("library")}
          aria-label="View library"
        >
          <Library size={22} strokeWidth={1.75} class="action-icon" />
          <span class="action-label">View library</span>
          <span class="action-sub">Explore files already on disk</span>
        </button>

        <button
          type="button"
          class="action-card"
          onclick={() => navigate("queue")}
          aria-label="Open queue"
        >
          <ListChecks size={22} strokeWidth={1.75} class="action-icon" />
          <span class="action-label">Open queue</span>
          <span class="action-sub">Track running and pending tasks</span>
        </button>
      </div>
    </section>

    <!-- ── Section 5: Recently queued ───────────────────────── -->
    {#if recentTasks.length > 0}
      <section class="recent-section" aria-label="Recently queued tasks">
        <div class="section-header">
          <h2 class="section-title">
            <Clock size={14} strokeWidth={1.75} class="section-title-icon" />
            Recently queued
          </h2>
          <button
            type="button"
            class="btn btn-ghost section-link"
            onclick={() => navigate("queue")}
          >
            View queue
            <ArrowUpRight size={13} strokeWidth={1.75} />
          </button>
        </div>

        <div class="recent-list" role="list">
          {#each recentTasks as task (task.id)}
            <div class="recent-row" role="listitem">
              <span class="recent-symbol tabnum">{task.symbol}</span>
              <span class="recent-kind">{kindLabel(task.kind)}</span>
              <span class="recent-date tabnum">{task.date}</span>
              <span class={statusPill(task.status)}>
                <span class="pill-dot" aria-hidden="true"></span>
                {task.status}
              </span>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <!-- Bottom padding -->
    <div class="bottom-pad" aria-hidden="true"></div>
  </div>
</div>

<style>
  /* ── Page shell ─────────────────────────────────────────────── */
  .home-page {
    height: 100%;
    overflow-y: auto;
    background: var(--bg);
  }

  .home-inner {
    max-width: 860px;
    margin: 0 auto;
    padding: var(--sp-10) var(--sp-8) 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-8);
  }

  /* ── Greeting ───────────────────────────────────────────────── */
  .greeting-section {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--sp-4);
    flex-wrap: wrap;
  }

  .greeting-body {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .greeting-title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    letter-spacing: -0.015em;
    line-height: 1.15;
    color: var(--fg);
  }

  .greeting-sub {
    font-size: var(--text-body);
    color: var(--fg-muted);
    max-width: 520px;
  }

  .cta-connect {
    height: 38px;
    padding: 0 var(--sp-5);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    border-radius: var(--r-md);
    gap: var(--sp-2);
    flex-shrink: 0;
  }

  /* ── Subscription card ──────────────────────────────────────── */
  .sub-card {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    padding: var(--sp-5) var(--sp-6);
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }

  .sub-card-header {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  .sub-icon {
    color: var(--fg-muted);
    display: inline-flex;
  }

  .sub-title {
    flex: 1;
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    color: var(--fg);
    letter-spacing: -0.005em;
  }

  .upgrade-btn-inline {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    padding: 3px var(--sp-3);
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: var(--r-pill);
    font-size: var(--text-caption);
    font-weight: var(--weight-semi);
    cursor: pointer;
    transition: filter var(--dur-fast) var(--ease-standard);
  }
  .upgrade-btn-inline:hover { filter: brightness(1.08); }
  .upgrade-btn-inline:active { filter: brightness(0.92); }

  .tier-pills-row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3);
  }

  .tier-pill {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-4);
    border-radius: var(--r-md);
    border: 1px solid transparent;
    min-width: 120px;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-1);
    padding: var(--sp-3) var(--sp-4);
  }

  .tier-pill-label {
    color: var(--fg-muted);
    display: block;
  }

  .tier-pill-value {
    font-size: var(--text-heading);
    font-weight: var(--weight-semi);
    letter-spacing: -0.005em;
    line-height: 1.2;
    display: block;
  }

  .tier-upgrade-mini {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    margin-top: var(--sp-1);
    padding: 2px 6px;
    border-radius: var(--r-pill);
    border: 1px solid currentColor;
    background: transparent;
    color: inherit;
    font-size: 10px;
    font-weight: var(--weight-semi);
    cursor: pointer;
    opacity: 0.8;
    transition: opacity var(--dur-fast) var(--ease-standard);
  }
  .tier-upgrade-mini:hover { opacity: 1; }

  /* Tier color variants — match KindGrid pill palette */
  .tier-unknown  { background: var(--surface-2);                     color: var(--fg-subtle);      border-color: var(--border); }
  .tier-free     { background: rgba(92, 101, 119, 0.15);             color: var(--fg-muted);       }
  .tier-value    { background: rgba(56, 132, 255, 0.10);             color: rgb(56, 132, 255);     border-color: rgba(56, 132, 255, 0.20); }
  .tier-standard { background: rgba(34, 175, 109, 0.12);             color: rgb(34, 175, 109);     border-color: rgba(34, 175, 109, 0.22); }
  .tier-pro      { background: rgba(244, 196, 48, 0.14);             color: rgb(212, 158, 0);      border-color: rgba(244, 196, 48, 0.30); }

  /* ── Section shared ─────────────────────────────────────────── */
  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--sp-4);
  }

  .section-title {
    font-size: var(--text-heading);
    font-weight: var(--weight-semi);
    color: var(--fg);
    letter-spacing: -0.005em;
    line-height: 1.4;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }

  :global(.section-title-icon) {
    color: var(--fg-subtle);
  }

  .section-link {
    height: 28px;
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    gap: var(--sp-1);
  }

  /* ── Library snapshot ───────────────────────────────────────── */
  .snapshot-section {
    display: flex;
    flex-direction: column;
  }

  .disk-stats {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-5);
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    margin-bottom: var(--sp-3);
    flex-wrap: wrap;
  }

  .disk-stat {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }

  :global(.stat-icon) {
    color: var(--fg-subtle);
    flex-shrink: 0;
  }

  .stat-value {
    font-size: var(--text-body);
    font-weight: var(--weight-semi);
    color: var(--fg);
  }

  .stat-label {
    color: var(--fg-subtle);
  }

  .disk-stat-divider {
    width: 1px;
    height: 20px;
    background: var(--border);
    flex-shrink: 0;
  }

  .coverage-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .coverage-row {
    display: grid;
    grid-template-columns: 80px 1fr 80px 64px;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) var(--sp-3);
    background: var(--surface-1);
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    cursor: pointer;
    text-align: left;
    transition:
      background var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard);
  }

  .coverage-row:hover {
    background: var(--surface-2);
    border-color: var(--border);
  }

  .coverage-row:focus-visible {
    box-shadow: var(--shadow-glow-accent);
    outline: none;
  }

  .cov-symbol {
    font-weight: var(--weight-semi);
    color: var(--fg);
    font-size: var(--text-body-sm);
    font-family: var(--font-mono);
  }

  .cov-kind {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
  }

  .cov-meta {
    font-size: var(--text-caption);
    color: var(--fg-subtle);
    text-transform: none;
    letter-spacing: 0;
    font-weight: var(--weight-normal);
    text-align: right;
  }

  .cov-size {
    font-size: var(--text-caption);
    color: var(--fg-subtle);
    text-transform: none;
    letter-spacing: 0;
    font-weight: var(--weight-normal);
    text-align: right;
  }

  .empty-hint {
    font-size: var(--text-body-sm);
    color: var(--fg-subtle);
    padding: var(--sp-4);
  }

  /* ── Quick actions ──────────────────────────────────────────── */
  .quick-actions {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }

  .action-row {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--sp-3);
  }

  .action-card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-2);
    padding: var(--sp-5);
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

  .action-card:hover {
    background: var(--surface-2);
    border-color: var(--border-strong);
    transform: translateY(-1px);
  }

  .action-card:focus-visible {
    box-shadow: var(--shadow-glow-accent);
  }

  .action-card:active {
    transform: translateY(0);
    background: var(--surface-3);
  }

  :global(.action-icon) {
    color: var(--accent);
    margin-bottom: var(--sp-1);
  }

  .action-label {
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    color: var(--fg);
    letter-spacing: -0.005em;
  }

  .action-sub {
    font-size: var(--text-caption);
    color: var(--fg-muted);
    text-transform: none;
    letter-spacing: 0;
    font-weight: var(--weight-normal);
    line-height: 1.4;
  }

  /* ── Recently queued ────────────────────────────────────────── */
  .recent-section {
    display: flex;
    flex-direction: column;
  }

  .recent-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .recent-row {
    display: grid;
    grid-template-columns: 72px 1fr 100px auto;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--r-sm);
    transition: background var(--dur-fast) var(--ease-standard);
  }

  .recent-row:hover {
    background: var(--surface-1);
  }

  .recent-symbol {
    font-family: var(--font-mono);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    color: var(--fg);
  }

  .recent-kind {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
  }

  .recent-date {
    font-size: var(--text-body-sm);
    color: var(--fg-subtle);
    text-align: right;
  }

  /* ── Bottom pad ─────────────────────────────────────────────── */
  .bottom-pad {
    height: var(--sp-12);
  }
</style>

<script lang="ts">
  /**
   * Inline composer popover. Bound to the global `composer` store so any
   * dataset card / detail page can open it with a known anchor dataset.
   * This is the modal-style overlay; cards anchor a small popover-flavoured
   * version inline (handled separately in DatasetCard).
   */
  import { onMount } from "svelte";
  import {
    X,
    ChevronRight,
    Loader2,
    Check,
    ChevronDown,
    Plus,
    Trash2,
  } from "lucide-svelte";
  import { ArrowUpRight, Lock } from "lucide-svelte";
  import { api, type EnqueueArgs, type Transforms } from "$lib/api";
  import { composer, closeComposer, app, tierForKind } from "$lib/stores/app.svelte";
  import SymbolPicker from "$lib/composer/SymbolPicker.svelte";
  import DateRangeSlider from "$lib/composer/DateRangeSlider.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";

  const verdict = $derived(tierForKind(composer.anchorDataset?.id ?? ""));
  const upgradeUrl = $derived(app.tierStatus?.upgrade_url ?? "https://thetadata.net/pricing");
  // `gated` means we KNOW the user is below the required tier. We
  // require both (a) tierStatus loaded AND (b) verdict.allowed=false so
  // we don't flash the upgrade CTA before tier_status arrives.
  const gated = $derived(!!app.tierStatus && !verdict.allowed);

  async function handleUpgrade() {
    try { await openUrl(upgradeUrl); } catch {}
  }

  let dialogEl = $state<HTMLDivElement | undefined>(undefined);

  function close() {
    closeComposer();
  }

  // Keep a derived dataset reference for label rendering.
  const ds = $derived(composer.anchorDataset);

  // Live trading dates for the (symbol, kind) pair powering the range
  // slider. Re-fetched whenever the symbol or kind changes.
  let availableDates = $state<string[]>([]);
  let datesLoading = $state(false);
  let lastFetchedKey = $state("");

  async function refreshAvailableDates() {
    if (!ds || !composer.symbol) {
      availableDates = [];
      return;
    }
    if (app.connState !== "connected") return;
    const key = `${ds.id}::${composer.symbol.trim().toUpperCase()}`;
    if (key === lastFetchedKey) return;
    lastFetchedKey = key;
    datesLoading = true;
    try {
      // Stock kinds → stock_list_dates(request_type, symbol).
      // Option kinds → option_list_dates(request_type, symbol). The
      // upstream request_type accepted is the underlying "TRADE" or
      // "QUOTE" kind for both endpoints.
      const isOption = ds.assetClass === "option";
      const endpoint = isOption ? "option_list_dates" : "stock_list_dates";
      // request_type: TRADE for any *_trade* kind, QUOTE otherwise. The
      // server uses this only as a presence-filter (does this date have
      // any rows of that kind?) — we want the broader set, so prefer
      // TRADE which is denser than QUOTE on most tiers.
      const request_type = ds.cadence === "quote" ? "QUOTE" : "TRADE";
      const args: Record<string, string> = {
        request_type,
        symbol: composer.symbol.trim().toUpperCase(),
      };
      const list = await api.listQuery({ endpoint, args });
      availableDates = list;
    } catch {
      availableDates = [];
    } finally {
      datesLoading = false;
    }
  }

  $effect(() => {
    // Track symbol changes (and dataset id) explicitly.
    void composer.symbol;
    void ds?.id;
    void app.connState;
    refreshAvailableDates();
  });

  // Multi-symbol parser: accept comma / whitespace / semicolon separated.
  function parseSymbols(raw: string): string[] {
    return raw
      .split(/[\s,;]+/)
      .map((s) => s.trim().toUpperCase())
      .filter(Boolean);
  }

  // ── Advanced helpers ──────────────────────────────────────
  function buildTransforms(): Transforms | null {
    const t: Transforms = {};
    // Strike unit: auto = leave alone (current SDK already gives dollars).
    // dollars = if data is in thousands, divide by 1000.
    // thousands = if data is in dollars, multiply by 1000.
    if (composer.strikeUnit === "dollars") {
      t.scale = { ...(t.scale ?? {}), strike: 0.001 };
    } else if (composer.strikeUnit === "thousands") {
      t.scale = { ...(t.scale ?? {}), strike: 1000 };
    }
    const rename: Record<string, string> = {};
    for (const r of composer.renames) {
      const from = r.from.trim();
      const to = r.to.trim();
      if (from && to && from !== to) rename[from] = to;
    }
    if (Object.keys(rename).length > 0) t.rename = rename;
    const drops = composer.drops.map((d) => d.trim()).filter(Boolean);
    if (drops.length > 0) t.drop = drops;
    return Object.keys(t).length > 0 ? t : null;
  }

  function addRename() {
    composer.renames = [...composer.renames, { from: "", to: "" }];
  }
  function removeRename(i: number) {
    composer.renames = composer.renames.filter((_, idx) => idx !== i);
  }

  function addDrop() {
    composer.drops = [...composer.drops, ""];
  }
  function removeDrop(i: number) {
    composer.drops = composer.drops.filter((_, idx) => idx !== i);
  }

  async function confirm() {
    if (composer.status === "queuing") return;
    const symbols = parseSymbols(composer.symbol);
    if (symbols.length === 0) { composer.msg = "Symbol required"; return; }
    if (!composer.start && !composer.end) {
      composer.msg = "Set a start date or a date range";
      return;
    }
    const baseArgs = {
      kind: ds?.id ?? "option_trade_quote",
      format: composer.format,
      interval: composer.interval,
      expiration: composer.expiration || "*",
      strike: composer.strike || "*",
      right: composer.right,
      start: composer.start || null,
      end: composer.end || null,
      date: !composer.end ? (composer.start || null) : null,
      transforms: buildTransforms(),
    };
    composer.status = "queuing";
    composer.msg = `Queueing ${symbols.length} symbol${symbols.length === 1 ? "" : "s"}…`;
    let totalTasks = 0;
    let firstErr = "";
    for (const symbol of symbols) {
      try {
        const n = await api.enqueue({ ...baseArgs, symbol } as EnqueueArgs);
        totalTasks += n;
      } catch (e: unknown) {
        if (!firstErr) firstErr = e instanceof Error ? e.message : String(e);
      }
    }
    if (firstErr && totalTasks === 0) {
      composer.status = "error";
      composer.msg = firstErr;
    } else {
      composer.status = "done";
      composer.msg = `Queued ${totalTasks} task${totalTasks === 1 ? "" : "s"} across ${symbols.length} symbol${symbols.length === 1 ? "" : "s"}${firstErr ? " (some failed)" : ""}`;
      setTimeout(close, 900);
    }
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === "Escape") close();
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") confirm();
  }

  onMount(() => {
    // Focus first input when opened.
    setTimeout(() => dialogEl?.querySelector<HTMLInputElement>("input")?.focus(), 30);
  });
</script>

{#if composer.open}
  <div
    class="composer-backdrop"
    role="presentation"
    onclick={close}
    onkeydown={handleKey}
  >
    <div
      class="composer"
      bind:this={dialogEl}
      role="dialog"
      aria-modal="true"
      aria-labelledby="composer-title"
      onclick={(e) => e.stopPropagation()}
      onkeydown={handleKey}
    >
      <header class="composer-header">
        <div class="composer-title-block">
          <span class="text-caption">Queue download</span>
          <h2 id="composer-title" class="composer-title">
            {ds?.title ?? "New download"}
          </h2>
          {#if ds}
            <p class="composer-sub fg-muted">{ds.subtitle}</p>
          {/if}
        </div>
        <button class="btn-icon" onclick={close} aria-label="Close">
          <X size={14} />
        </button>
      </header>

      {#if gated}
        <div class="tier-banner" role="alert">
          <Lock size={13} strokeWidth={2} />
          <div class="tier-banner-text">
            <strong>Requires {verdict.required} tier.</strong>
            You're on <span class="tabnum">{verdict.user}</span>. Upgrade your
            ThetaData subscription to queue this dataset.
          </div>
          <button class="btn-upgrade" onclick={handleUpgrade}>
            <ArrowUpRight size={12} />
            Upgrade
          </button>
        </div>
      {/if}

      <div class="composer-form">
        <label class="field-stack">
          <span class="text-caption">
            Symbol{parseSymbols(composer.symbol).length > 1 ? `s (${parseSymbols(composer.symbol).length})` : ""}
          </span>
          <SymbolPicker
            bind:value={composer.symbol}
            assetClass={ds?.assetClass ?? "stock"}
            placeholder={ds?.assetClass === "option" ? "Underlying root, or QQQ, SPY, IWM" : "QQQ — or comma-separate: QQQ, SPY, AAPL"}
            autofocus
          />
          <span class="hint fg-muted">Type one symbol or paste a comma / space-separated list to bulk-queue.</span>
        </label>

        <DateRangeSlider
          bind:start={composer.start}
          bind:end={composer.end}
          {availableDates}
          label={datesLoading ? "Date range — loading available days…" : "Date range"}
        />

        <div class="row-2">
          <label class="field-stack">
            <span class="text-caption">Format</span>
            <select class="field-input" bind:value={composer.format}>
              <option value="parquet">Parquet (zstd)</option>
              <option value="csv">CSV</option>
              <option value="jsonl">JSON Lines</option>
              <option value="json">JSON array</option>
            </select>
          </label>
          {#if ds?.cadence === "quote"}
            <label class="field-stack">
              <span class="text-caption">Quote interval</span>
              <select class="field-input" bind:value={composer.interval}>
                <option value="0">0 — tick-by-tick</option>
                <option value="1s">1s — sampled</option>
                <option value="60s">60s — 1m sampled</option>
              </select>
            </label>
          {:else}
            <label class="field-stack">
              <span class="text-caption">Priority</span>
              <select class="field-input" disabled>
                <option>Normal</option>
              </select>
            </label>
          {/if}
        </div>

        {#if ds?.assetClass === "option"}
          <div class="row-3">
            <label class="field-stack">
              <span class="text-caption">Expiration</span>
              <input class="field-input" bind:value={composer.expiration} />
            </label>
            <label class="field-stack">
              <span class="text-caption">Strike</span>
              <input class="field-input" bind:value={composer.strike} />
            </label>
            <label class="field-stack">
              <span class="text-caption">Right</span>
              <select class="field-input" bind:value={composer.right}>
                <option value="both">Both</option>
                <option value="C">Calls</option>
                <option value="P">Puts</option>
              </select>
            </label>
          </div>
        {/if}

        <!-- ── Advanced (collapsible) ───────────────────────── -->
        <div class="advanced">
          <button
            type="button"
            class="advanced-toggle"
            onclick={() => (composer.advancedOpen = !composer.advancedOpen)}
            aria-expanded={composer.advancedOpen}
          >
            <ChevronDown size={14} class={composer.advancedOpen ? "open" : ""} />
            <span>Advanced — field renames, units, drops</span>
          </button>
          {#if composer.advancedOpen}
            <div class="advanced-body">
              <label class="field-stack">
                <span class="text-caption">Strike unit</span>
                <select class="field-input" bind:value={composer.strikeUnit}>
                  <option value="auto">Auto (leave as upstream)</option>
                  <option value="dollars">Force dollars (÷ 1000 if thousands)</option>
                  <option value="thousands">Force thousands (× 1000 if dollars)</option>
                </select>
                <span class="hint fg-muted">
                  Recent thetadatadx versions decode strike to dollars (f64).
                  Use "Force dollars" if your downstream tools expect dollar
                  amounts and you're hitting an older endpoint that emits
                  raw thousands-of-cents integers.
                </span>
              </label>

              <div class="adv-section">
                <div class="adv-section-head">
                  <span class="text-caption">Field renames</span>
                  <button class="row-btn" onclick={addRename}>
                    <Plus size={11} /> Add
                  </button>
                </div>
                {#if composer.renames.length === 0}
                  <p class="hint fg-muted">
                    Map upstream column names to your preferred output names.
                    Examples: <code>strike → strike_dollars</code>,
                    <code>ms_of_day → ts_ms</code>.
                  </p>
                {/if}
                {#each composer.renames as r, i}
                  <div class="rename-row">
                    <input class="field-input text-mono" placeholder="upstream"
                           bind:value={r.from} />
                    <ChevronRight size={12} class="arrow" />
                    <input class="field-input text-mono" placeholder="renamed"
                           bind:value={r.to} />
                    <button class="row-btn ghost" onclick={() => removeRename(i)} aria-label="Remove">
                      <Trash2 size={11} />
                    </button>
                  </div>
                {/each}
              </div>

              <div class="adv-section">
                <div class="adv-section-head">
                  <span class="text-caption">Drop columns</span>
                  <button class="row-btn" onclick={addDrop}>
                    <Plus size={11} /> Add
                  </button>
                </div>
                {#if composer.drops.length === 0}
                  <p class="hint fg-muted">
                    List columns to omit from the output. Useful for trimming
                    sequence / condition_flags / venue from quote-heavy
                    parquet files.
                  </p>
                {/if}
                {#each composer.drops as d, i}
                  <div class="drop-row">
                    <input class="field-input text-mono" placeholder="column_name"
                           bind:value={composer.drops[i]} />
                    <button class="row-btn ghost" onclick={() => removeDrop(i)} aria-label="Remove">
                      <Trash2 size={11} />
                    </button>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      </div>

      <footer class="composer-footer">
        <span class="composer-msg" class:error={composer.status === "error"}>
          {composer.msg}
        </span>
        <div class="composer-actions">
          <button class="btn btn-ghost" onclick={close}>Cancel</button>
          {#if gated}
            <button class="btn btn-primary upgrade-action" onclick={handleUpgrade}>
              Upgrade to {verdict.required}
              <ArrowUpRight size={14} />
            </button>
          {:else}
            <button
              class="btn btn-primary"
              onclick={confirm}
              disabled={composer.status === "queuing"}
            >
              {#if composer.status === "queuing"}
                <Loader2 class="spin" size={14} />
                Queueing…
              {:else if composer.status === "done"}
                <Check size={14} />
                Queued
              {:else}
                Queue
                <ChevronRight size={14} />
              {/if}
            </button>
          {/if}
        </div>
      </footer>
    </div>
  </div>
{/if}

<style>
  .composer-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 11, 18, 0.55);
    backdrop-filter: blur(4px);
    -webkit-backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    animation: fade-in var(--dur-base) var(--ease-standard);
  }
  @keyframes fade-in { from { opacity: 0; } to { opacity: 1; } }

  .composer {
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-modal);
    width: 560px;
    max-width: calc(100vw - var(--sp-8));
    display: flex;
    flex-direction: column;
    animation: pop-in var(--dur-base) var(--ease-standard);
  }
  @keyframes pop-in {
    from { opacity: 0; transform: translateY(8px) scale(0.98); }
    to   { opacity: 1; transform: translateY(0)    scale(1);   }
  }

  .composer-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: var(--sp-3);
    padding: var(--sp-5) var(--sp-5) var(--sp-4);
    border-bottom: 1px solid var(--border);
  }
  .composer-title-block {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .composer-title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    letter-spacing: -0.015em;
    color: var(--fg);
    margin: 0;
  }
  .composer-sub {
    font-size: var(--text-body-sm);
    margin: 0;
  }

  .composer-form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-5);
  }

  .field-stack {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .row-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-3);
  }
  .row-3 {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: var(--sp-3);
  }

  .composer-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-4) var(--sp-5);
    border-top: 1px solid var(--border);
    background: var(--surface-1);
    border-radius: 0 0 var(--r-lg) var(--r-lg);
  }
  .composer-msg {
    color: var(--fg-muted);
    font-size: var(--text-body-sm);
  }
  .composer-msg.error { color: var(--bad); }
  .composer-actions {
    display: flex;
    gap: var(--sp-2);
  }
  :global(.spin) { animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* ── Advanced section ─────────────────────────────────── */
  .advanced {
    border-top: 1px solid var(--border);
    padding-top: var(--sp-3);
    margin-top: var(--sp-2);
  }
  .advanced-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 0;
    background: transparent;
    border: 0;
    color: var(--fg-muted);
    cursor: pointer;
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
  }
  .advanced-toggle:hover { color: var(--fg); }
  .advanced-toggle :global(.open) { transform: rotate(0deg); }
  .advanced-toggle :global(svg) { transform: rotate(-90deg); transition: transform var(--dur-fast) var(--ease-standard); }
  .advanced-toggle[aria-expanded="true"] :global(svg) { transform: rotate(0deg); }
  .advanced-body {
    margin-top: var(--sp-3);
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-3);
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
  }
  .adv-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .adv-section-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .rename-row, .drop-row {
    display: grid;
    grid-template-columns: 1fr auto 1fr auto;
    gap: 6px;
    align-items: center;
  }
  .drop-row { grid-template-columns: 1fr auto; }
  .rename-row :global(.arrow) { color: var(--fg-subtle); }
  .row-btn {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 3px 8px;
    background: var(--surface-3);
    border: 1px solid var(--border);
    color: var(--fg-muted);
    border-radius: var(--r-sm);
    font-size: 11px;
    cursor: pointer;
  }
  .row-btn:hover { background: var(--accent-tint); color: var(--accent-hi); border-color: rgba(124,140,255,0.3); }
  .row-btn.ghost { background: transparent; }
  .hint code {
    font-family: var(--font-mono);
    background: var(--surface-3);
    padding: 0 4px;
    border-radius: 3px;
    font-size: 11px;
  }

  /* Tier-gated banner above the form when user is below required tier. */
  .tier-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0 var(--sp-5);
    padding: 10px 12px;
    background: rgba(244, 196, 48, 0.10);
    border: 1px solid rgba(244, 196, 48, 0.32);
    border-radius: var(--r-md);
    color: rgb(212, 158, 0);
    font-size: var(--text-body-sm);
  }
  .tier-banner-text { flex: 1; line-height: 1.4; color: var(--fg); }
  .tier-banner-text strong { color: var(--fg); }
  .btn-upgrade {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    border-radius: var(--r-sm);
    border: 1px solid var(--accent, rgb(56, 132, 255));
    background: var(--accent, rgb(56, 132, 255));
    color: white;
    font-weight: 600;
    font-size: var(--text-body-sm);
    cursor: pointer;
    white-space: nowrap;
  }
  .btn-upgrade:hover { filter: brightness(1.08); }
  .upgrade-action :global(svg) { stroke-width: 2; }
</style>

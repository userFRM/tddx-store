<script lang="ts">
  /**
   * Step 2 — Which symbols?
   * Three modes: Symbols (chip input with autocomplete — one ticker or
   * fifty, typed or pasted) · Watchlist (saved in Settings) · Index
   * preset (live constituents).
   * Emits a flat string[] of uppercase ticker symbols to the parent.
   */
  import { onMount } from "svelte";
  import { Tags, Layers, Loader2, Star } from "lucide-svelte";
  import SymbolChips from "$lib/composer/SymbolChips.svelte";
  import BetaTag from "$lib/feedback/BetaTag.svelte";
  import IndexPresetNotice from "$lib/catalogue/IndexPresetNotice.svelte";
  import { api, TAURI_AVAILABLE } from "$lib/api";
  import { app, log, navigate } from "$lib/stores/app.svelte";
  import type { AssetClass } from "$lib/stores/app.svelte";
  import type { IndexPresetView } from "$lib/api";

  let {
    assetClass,
    symbols = $bindable<string[]>([]),
  }: {
    assetClass: AssetClass;
    symbols: string[];
  } = $props();

  type Mode = "symbols" | "watchlist" | "preset";
  let mode = $state<Mode>("symbols");

  // Symbols mode: one ticker or many, as chips. This replaced a single-
  // symbol autocomplete and a separate paste box, which forced a choice
  // between "autocomplete" and "more than one" that nobody should have
  // to make.
  let chosen = $state<string[]>([]);

  // Index preset mode
  let presets = $state<IndexPresetView[]>([]);
  let presetsLoading = $state(false);
  let selectedPresetId = $state("");
  // Constituents are lazy — `index_presets` returns just the labels;
  // the actual ticker list lives behind `index_constituents(id)` so
  // we only hit the upstream sponsor CDN when the user picks a preset.
  let constituentsCache = $state<Record<string, string[]>>({});
  let constituentsLoading = $state(false);
  let constituentsError = $state("");


  // Watchlist mode — named lists kept in Settings, so the same dozen
  // tickers are one click rather than retyped every time.
  const watchlists = $derived(app.settings.preferences?.watchlists ?? []);
  let selectedWatchlist = $state(0);


  // Sync outbound symbols whenever inputs change
  $effect(() => {
    if (mode === "symbols") {
      symbols = [...chosen];
    } else if (mode === "watchlist") {
      symbols = [...(watchlists[selectedWatchlist]?.symbols ?? [])];
    } else if (mode === "preset") {
      const cached = constituentsCache[selectedPresetId];
      symbols = cached ? [...cached] : [];
    }
  });

  async function loadPresets() {
    if (presetsLoading || presets.length > 0) return;
    if (!TAURI_AVAILABLE) return;
    presetsLoading = true;
    try {
      presets = await api.indexPresets();
      if (presets.length > 0 && !selectedPresetId) {
        selectedPresetId = presets[0].id;
      }
    } catch (e: unknown) {
      log("warn", `index presets unavailable: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      presetsLoading = false;
    }
  }

  async function loadConstituents(id: string) {
    if (!id || constituentsCache[id] || constituentsLoading) return;
    if (!TAURI_AVAILABLE) return;
    constituentsLoading = true;
    constituentsError = "";
    try {
      const list = await api.indexConstituents(id);
      constituentsCache = { ...constituentsCache, [id]: list };
      log("info", `Loaded ${list.length} constituents for ${id}`);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      constituentsError = msg;
      log("warn", `index_constituents(${id}) failed: ${msg}`);
    } finally {
      constituentsLoading = false;
    }
  }

  $effect(() => {
    if (mode === "preset") loadPresets();
  });

  // Whenever the user picks a different preset (or first lands here),
  // make sure its constituents are fetched. The cache means switching
  // back to a previously-loaded preset is instant.
  $effect(() => {
    if (mode === "preset" && selectedPresetId) {
      void loadConstituents(selectedPresetId);
    }
  });

  onMount(() => {
    if (mode === "preset") loadPresets();
  });

  const MODES: { id: Mode; label: string; icon: typeof Tags; description: string; beta?: boolean }[] = [
    { id: "symbols",   label: "Symbols",       icon: Tags,   description: "One or many, with autocomplete" },
    { id: "watchlist", label: "Watchlist",     icon: Star,   description: "A list saved in Settings" },
    { id: "preset",    label: "Index preset",  icon: Layers, description: "S&P 500, Nasdaq-100, etc.", beta: true },
  ];

  const countLabel = $derived(
    symbols.length === 0 ? "No symbols selected"
    : symbols.length === 1 ? `1 symbol: ${symbols[0]}`
    : `${symbols.length.toLocaleString()} symbols`
  );
</script>

<div class="universe">
  <!-- Mode radio cards -->
  <div class="mode-row" role="radiogroup" aria-label="Symbol selection mode">
    {#each MODES as m}
      {@const Icon = m.icon}
      <button
        type="button"
        role="radio"
        aria-checked={mode === m.id}
        class="mode-card"
        class:selected={mode === m.id}
        onclick={() => (mode = m.id)}
      >
        <Icon size={15} strokeWidth={1.75} />
        <span class="mode-label">{m.label}{#if m.beta}<BetaTag />{/if}</span>
        <span class="mode-desc">{m.description}</span>
      </button>
    {/each}
  </div>

  <!-- Input area for the selected mode -->
  <div class="input-area">
    {#if mode === "symbols"}
      <SymbolChips bind:symbols={chosen} {assetClass}
        placeholder={assetClass === "option" ? "Add option roots — e.g. SPX, SPY" : "Add symbols — e.g. SPY, QQQ, AAPL"} />

    {:else if mode === "watchlist"}
      {#if watchlists.length === 0}
        <p class="empty-hint">
          No watchlists yet.
          <button type="button" class="link-btn" onclick={() => navigate("settings")}>
            Create one in Settings
          </button>
          to reuse a set of tickers.
        </p>
      {:else}
        <div class="preset-picker">
          <select class="field-input" bind:value={selectedWatchlist}>
            {#each watchlists as w, i (i)}
              <option value={i}>{w.name} · {w.symbols.length} symbol{w.symbols.length === 1 ? "" : "s"}</option>
            {/each}
          </select>
        </div>
      {/if}

    {:else if mode === "preset"}
      {#if presetsLoading}
        <div class="loading-row">
          <Loader2 size={14} class="spin" />
          <span>Loading index constituents…</span>
        </div>
      {:else if presets.length === 0}
        <p class="empty-hint">
          Connect to ThetaData to load index preset lists.
        </p>
      {:else}
        <div class="preset-picker">
          <IndexPresetNotice />
          <select
            class="field-input"
            bind:value={selectedPresetId}
          >
            {#each presets as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
          {#if selectedPresetId}
            {@const found = presets.find((x) => x.id === selectedPresetId)}
            {#if found}
              <p class="preset-desc">{found.description}</p>
            {/if}
            {#if constituentsLoading}
              <div class="loading-row">
                <Loader2 size={14} class="spin" />
                <span>Fetching live constituents…</span>
              </div>
            {:else if constituentsError}
              <p class="empty-hint">
                Couldn't load constituents — {constituentsError}.
                Switch to Custom list to paste tickers manually.
              </p>
            {/if}
          {/if}
        </div>
      {/if}

    {/if}
  </div>

  <!-- Live symbol count -->
  <div class="symbol-count" aria-live="polite">
    <span class="count-label tabnum" class:has-symbols={symbols.length > 0}>
      {countLabel}
    </span>
    {#if mode !== "symbols" && symbols.length > 0}
      <span class="sample-preview">
        {symbols.slice(0, 6).join(", ")}{symbols.length > 6 ? ` +${symbols.length - 6} more` : ""}
      </span>
    {/if}
  </div>
</div>

<style>
  .universe {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .mode-row {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--sp-2);
  }

  .mode-card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 3px;
    padding: var(--sp-3);
    background: var(--surface-1);
    border: 1.5px solid var(--border);
    border-radius: var(--r-md);
    cursor: pointer;
    text-align: left;
    transition:
      background var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard);
    outline: none;
    color: var(--fg-muted);
  }

  .mode-card:hover:not(.selected) {
    background: var(--surface-2);
    border-color: var(--border-strong);
    color: var(--fg);
  }

  .mode-card:focus-visible {
    box-shadow: var(--shadow-glow-accent);
  }

  .mode-card.selected {
    background: var(--accent-tint);
    border-color: var(--accent);
    color: var(--accent-hi);
  }

  .mode-label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    color: inherit;
  }

  .mode-desc {
    font-size: var(--text-caption);
    text-transform: none;
    letter-spacing: 0;
    font-weight: var(--weight-normal);
    color: var(--fg-muted);
    line-height: 1.3;
  }

  .mode-card.selected .mode-desc {
    color: var(--accent);
    opacity: 0.8;
  }

  .input-area {
    min-height: 60px;
  }

  .loading-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    color: var(--fg-muted);
    font-size: var(--text-body-sm);
    padding: var(--sp-3);
  }

  .empty-hint {
    font-size: var(--text-body-sm);
    color: var(--fg-subtle);
    padding: var(--sp-3);
    border: 1px dashed var(--border);
    border-radius: var(--r-sm);
  }

  .link-btn {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
    font: inherit;
    text-decoration: underline;
  }
  .preset-picker {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .preset-desc {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    padding: 0 var(--sp-1);
  }





  .symbol-count {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) var(--sp-3);
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    min-height: 32px;
  }

  .count-label {
    font-size: var(--text-body-sm);
    color: var(--fg-subtle);
    font-weight: var(--weight-medium);
    transition: color var(--dur-fast) var(--ease-standard);
  }

  .count-label.has-symbols {
    color: var(--good);
  }

  .sample-preview {
    font-family: var(--font-mono);
    font-size: var(--text-caption);
    font-variant-numeric: tabular-nums;
    color: var(--fg-muted);
    text-transform: uppercase;
    letter-spacing: 0.02em;
  }
</style>

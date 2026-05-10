<script lang="ts">
  /**
   * Step 2 — Which symbols?
   * Three modes: Single (autocomplete) · Index preset (dropdown) · Custom list (textarea).
   * Emits a flat string[] of uppercase ticker symbols to the parent.
   */
  import { onMount } from "svelte";
  import { User, Layers, List, Loader2 } from "lucide-svelte";
  import SymbolPicker from "$lib/composer/SymbolPicker.svelte";
  import { api, TAURI_AVAILABLE } from "$lib/api";
  import { log } from "$lib/stores/app.svelte";
  import type { AssetClass } from "$lib/stores/app.svelte";
  import type { IndexPresetView } from "$lib/api";

  let {
    assetClass,
    symbols = $bindable<string[]>([]),
  }: {
    assetClass: AssetClass;
    symbols: string[];
  } = $props();

  type Mode = "single" | "preset" | "custom";
  let mode = $state<Mode>("single");

  // Single mode
  let singleSymbol = $state("");

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

  // Custom list mode
  let customRaw = $state("");

  function parseCustom(raw: string): string[] {
    return raw
      .split(/[\s,;\n]+/)
      .map((s) => s.trim().toUpperCase())
      .filter((s) => s.length > 0);
  }

  // Sync outbound symbols whenever inputs change
  $effect(() => {
    if (mode === "single") {
      const s = singleSymbol.trim().toUpperCase();
      symbols = s ? [s] : [];
    } else if (mode === "preset") {
      const cached = constituentsCache[selectedPresetId];
      symbols = cached ? [...cached] : [];
    } else {
      symbols = parseCustom(customRaw);
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

  const MODES: { id: Mode; label: string; icon: typeof User; description: string }[] = [
    { id: "single", label: "Single symbol",  icon: User,   description: "One ticker with autocomplete" },
    { id: "preset", label: "Index preset",   icon: Layers, description: "S&P 500, Nasdaq-100, etc." },
    { id: "custom", label: "Custom list",    icon: List,   description: "Paste any ticker list" },
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
        <span class="mode-label">{m.label}</span>
        <span class="mode-desc">{m.description}</span>
      </button>
    {/each}
  </div>

  <!-- Input area for the selected mode -->
  <div class="input-area">
    {#if mode === "single"}
      <SymbolPicker
        bind:value={singleSymbol}
        {assetClass}
        placeholder={assetClass === "option" ? "Option root, e.g. SPX" : "e.g. QQQ, SPY, AAPL"}
        autofocus={false}
      />

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

    {:else}
      <textarea
        class="custom-textarea"
        bind:value={customRaw}
        placeholder={"AAPL, MSFT, GOOGL\nQQQ\nSPY, IWM, TLT"}
        rows={5}
        spellcheck={false}
      ></textarea>
      <p class="custom-hint">Comma, newline, or space separated. Duplicates are removed automatically.</p>
    {/if}
  </div>

  <!-- Live symbol count -->
  <div class="symbol-count" aria-live="polite">
    <span class="count-label tabnum" class:has-symbols={symbols.length > 0}>
      {countLabel}
    </span>
    {#if mode === "custom" && symbols.length > 0}
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

  .custom-textarea {
    width: 100%;
    padding: var(--sp-3);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: var(--text-body-sm);
    font-variant-numeric: tabular-nums;
    resize: vertical;
    outline: none;
    line-height: 1.6;
    transition: border-color var(--dur-fast) var(--ease-standard),
                box-shadow var(--dur-fast) var(--ease-standard);
  }

  .custom-textarea:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-tint);
  }

  .custom-textarea::placeholder {
    color: var(--fg-subtle);
    font-family: var(--font-ui);
    font-variant-numeric: normal;
  }

  .custom-hint {
    font-size: var(--text-caption);
    color: var(--fg-subtle);
    text-transform: none;
    letter-spacing: 0;
    font-weight: var(--weight-normal);
    margin-top: var(--sp-1);
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

<script lang="ts">
  /**
   * Autocomplete symbol picker. Backed by the live ThetaData list endpoints
   * via `list_query`. Lazy-loads + caches per asset class so dropdowns open
   * instantly after first use.
   *
   * Keyboard:
   *   - typing filters the suggestion list
   *   - ArrowDown / ArrowUp move highlight
   *   - Enter / Tab commits highlighted suggestion
   *   - Esc closes
   */
  import { onDestroy } from "svelte";
  import { Search, X, Loader2 } from "lucide-svelte";
  import { api } from "$lib/api";
  import { app, warmCaches, type AssetClass } from "$lib/stores/app.svelte";

  let {
    value = $bindable(""),
    assetClass = "stock" as AssetClass,
    placeholder = "Symbol (e.g. QQQ)",
    autofocus = false,
  }: {
    value: string;
    assetClass?: AssetClass;
    placeholder?: string;
    autofocus?: boolean;
  } = $props();

  // Symbol lists come from the global `app.symbols` cache populated by
  // `warmCaches()` on connect. The picker is a thin, reactive view —
  // no per-component fetch, no per-component sort. If the cache is
  // empty (pre-connect, or warmCaches in flight), we kick a refresh
  // and show the loading state until it lands.
  type LocalCache = { loading: boolean; symbols: string[] };
  let lazyCache = $state<Record<AssetClass, LocalCache>>({
    stock:  { loading: false, symbols: [] },
    option: { loading: false, symbols: [] },
    index:  { loading: false, symbols: [] },
    rate:   { loading: false, symbols: [] },
  });

  let open = $state(false);
  let highlight = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  /** Reactive view of the symbol list for the current asset class.
   *  Stocks + options come from the warmed `app.symbols`; index +
   *  rate fall back to a per-class lazy cache (no warm endpoint yet). */
  const cache = $derived.by<LocalCache>(() => {
    // Combo = stock symbols that also have an option chain. Intersect
    // stockSymbols with optionRoots so the picker shows only tickers
    // where both kinds resolve.
    if (assetClass === "stock") {
      return {
        loading: app.symbols.loading && app.symbols.stockSymbols.length === 0,
        symbols: app.symbols.stockSymbols,
      };
    }
    if (assetClass === "option") {
      return {
        loading: app.symbols.loading && app.symbols.optionRoots.length === 0,
        symbols: app.symbols.optionRoots,
      };
    }
    return lazyCache[assetClass];
  });

  async function loadOnce() {
    if (assetClass === "stock" || assetClass === "option") {
      // Trigger the global warm on first focus if it hasn't run yet.
      if (
        app.symbols.loadedAt === null &&
        !app.symbols.loading &&
        app.connState === "connected"
      ) {
        void warmCaches();
      }
      return;
    }
    const c = lazyCache[assetClass];
    if (c.loading || c.symbols.length > 0) return;
    c.loading = true;
    lazyCache = { ...lazyCache };
    try {
      const endpoint =
        assetClass === "index" ? "index_list_symbols" : "stock_list_symbols";
      const list = await api.listQuery({ endpoint, args: {} });
      c.symbols = list.sort();
    } catch {
      // Pre-connect or unsupported tier — leave cache empty so the
      // dropdown gracefully falls back to "type freely".
    } finally {
      c.loading = false;
      lazyCache = { ...lazyCache };
    }
  }

  function onFocus() {
    open = true;
    loadOnce();
  }

  function onBlur(e: FocusEvent) {
    // Defer close so a click on a suggestion still registers.
    setTimeout(() => {
      const root = (inputEl?.parentElement?.parentElement) ?? null;
      if (root && document.activeElement && root.contains(document.activeElement)) return;
      open = false;
    }, 120);
  }

  const filtered = $derived.by(() => {
    const q = value.trim().toUpperCase();
    if (cache.symbols.length === 0) return [];
    if (q === "") return cache.symbols.slice(0, 32);
    const starts: string[] = [];
    const contains: string[] = [];
    for (const s of cache.symbols) {
      if (s.startsWith(q)) starts.push(s);
      else if (s.includes(q)) contains.push(s);
      if (starts.length + contains.length >= 64) break;
    }
    return [...starts, ...contains].slice(0, 32);
  });

  function pick(sym: string) {
    value = sym;
    open = false;
    highlight = 0;
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      open = true;
      highlight = Math.min(highlight + 1, filtered.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      highlight = Math.max(highlight - 1, 0);
    } else if (e.key === "Enter" || e.key === "Tab") {
      if (open && filtered[highlight]) {
        e.preventDefault();
        pick(filtered[highlight]);
      }
    } else if (e.key === "Escape") {
      open = false;
    }
  }

  function clear() {
    value = "";
    inputEl?.focus();
  }

  $effect(() => {
    if (autofocus && inputEl) inputEl.focus();
  });

  onDestroy(() => { open = false; });
</script>

<div class="picker">
  <div class="input-shell">
    <Search size={14} class="picker-icon" />
    <input
      bind:this={inputEl}
      bind:value
      class="picker-input"
      type="text"
      autocomplete="off"
      spellcheck="false"
      {placeholder}
      onfocus={onFocus}
      onblur={onBlur}
      onkeydown={onKey}
      oninput={() => { open = true; highlight = 0; }}
    />
    {#if value.length > 0}
      <button class="picker-clear" onclick={clear} aria-label="Clear">
        <X size={12} />
      </button>
    {/if}
  </div>

  {#if open}
    <div class="picker-pop" role="listbox">
      {#if cache.loading}
        <div class="picker-row loading">
          <Loader2 class="spin" size={12} />
          <span>Loading {assetClass} symbols…</span>
        </div>
      {:else if filtered.length === 0}
        {#if cache.symbols.length === 0 && !cache.loading}
          <div class="picker-row muted">
            <span>Connect to ThetaData to see live symbol list. You can still type any symbol manually.</span>
          </div>
        {:else}
          <div class="picker-row muted">
            <span>No matches.</span>
          </div>
        {/if}
      {:else}
        {#each filtered as sym, i (sym)}
          <button
            type="button"
            class="picker-row"
            class:active={i === highlight}
            role="option"
            aria-selected={i === highlight}
            onmouseenter={() => (highlight = i)}
            onclick={() => pick(sym)}
          >
            <span class="sym">{sym}</span>
          </button>
        {/each}
        {#if filtered.length === 32}
          <div class="picker-row muted">
            <span>… {cache.symbols.length - 32} more — keep typing</span>
          </div>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .picker { position: relative; width: 100%; }

  .input-shell {
    position: relative;
    display: flex;
    align-items: center;
  }
  .input-shell :global(.picker-icon) {
    position: absolute;
    left: var(--sp-3);
    color: var(--fg-subtle);
    pointer-events: none;
  }
  .picker-input {
    height: 32px;
    width: 100%;
    padding: 0 32px 0 32px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    color: var(--fg);
    font-size: var(--text-body-sm);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    text-transform: uppercase;
    letter-spacing: 0.02em;
    outline: none;
    transition: border-color var(--dur-fast) var(--ease-standard),
                box-shadow var(--dur-fast) var(--ease-standard);
  }
  .picker-input::placeholder {
    color: var(--fg-subtle);
    text-transform: none;
    font-family: var(--font-ui);
    letter-spacing: normal;
  }
  .picker-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-tint);
  }

  .picker-clear {
    position: absolute;
    right: 6px;
    top: 50%;
    transform: translateY(-50%);
    background: transparent;
    border: 0;
    color: var(--fg-subtle);
    cursor: pointer;
    padding: 4px;
    border-radius: var(--r-sm);
    display: inline-flex;
  }
  .picker-clear:hover { color: var(--fg); background: var(--surface-3); }

  .picker-pop {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    max-height: 280px;
    overflow-y: auto;
    background: var(--surface-3);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-sm);
    box-shadow: var(--shadow-modal);
    padding: 4px;
    z-index: 50;
  }
  .picker-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: 6px var(--sp-2);
    width: 100%;
    text-align: left;
    background: transparent;
    border: 0;
    border-radius: var(--r-sm);
    color: var(--fg);
    font-size: var(--text-body-sm);
    cursor: pointer;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  .picker-row.active { background: var(--accent-tint); color: var(--accent-hi); }
  .picker-row.muted, .picker-row.loading {
    color: var(--fg-muted);
    font-family: var(--font-ui);
    font-variant-numeric: normal;
    cursor: default;
  }
  :global(.spin) { animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>

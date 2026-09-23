<script lang="ts">
  /**
   * Pick any number of symbols as removable chips, with autocomplete
   * that comes back for the next one.
   *
   * Commit a symbol with Enter, Tab, comma or space; the dropdown stays
   * open for the next. Backspace on an empty field removes the last
   * chip. Pasting "AAPL, MSFT NVDA" becomes three chips. A symbol the
   * asset class does not list is still accepted — the cache can lag
   * a new listing — but is marked, so a typo does not become a
   * download that fails with no data.
   */
  import { X, Loader2, AlertTriangle } from "lucide-svelte";
  import { api } from "$lib/api";
  import { app, warmCaches, loadCoverage, type AssetClass } from "$lib/stores/app.svelte";

  let {
    symbols = $bindable<string[]>([]),
    assetClass = "stock" as AssetClass,
    placeholder = "Add symbols — e.g. SPY, QQQ",
  }: {
    symbols: string[];
    assetClass?: AssetClass;
    placeholder?: string;
  } = $props();

  // ── Universe ────────────────────────────────────────────────
  // Stocks and option roots come from the caches warmed on connect;
  // indices and rates are fetched once, on first use.
  const lazy = $state<Record<string, { loading: boolean; list: string[] }>>({
    index: { loading: false, list: [] },
    rate: { loading: false, list: [] },
  });

  const universe = $derived.by<{ loading: boolean; list: string[] }>(() => {
    if (assetClass === "stock") {
      return { loading: app.symbols.loading && !app.symbols.stockSymbols.length, list: app.symbols.stockSymbols };
    }
    if (assetClass === "option") {
      return { loading: app.symbols.loading && !app.symbols.optionRoots.length, list: app.symbols.optionRoots };
    }
    return lazy[assetClass];
  });
  const known = $derived(new Set(universe.list));

  async function ensureUniverse() {
    // The library feeds the ranking; it is cheap once loaded.
    void loadCoverage();
    if (assetClass === "stock" || assetClass === "option") {
      if (app.symbols.loadedAt === null && !app.symbols.loading && app.connState === "connected") {
        void warmCaches();
      }
      return;
    }
    const c = lazy[assetClass];
    if (c.loading || c.list.length) return;
    c.loading = true;
    try {
      const endpoint = assetClass === "index" ? "index_list_symbols" : "stock_list_symbols";
      c.list = (await api.listQuery({ endpoint, args: {} })).sort();
    } catch {
      // Not connected, or not on this plan: type freely.
    } finally {
      c.loading = false;
    }
  }

  // ── Input ───────────────────────────────────────────────────
  let draft = $state("");
  let open = $state(false);
  let highlight = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  const LIMIT = 30;

  /** Symbols this user already works with: what is on disk, what is in
   *  a watchlist, what they downloaded recently. Alphabetical order put
   *  QQA and QQAU above QQQ; nobody typing "QQ" means QQA. */
  const familiar = $derived(
    new Set([
      ...app.coverage.map((c) => c.symbol),
      ...(app.settings.preferences?.watchlists ?? []).flatMap((w) => w.symbols),
      ...app.batches.map((b) => b.first_symbol),
    ]),
  );

  // Highlight follows the mouse only when it moves. `mouseenter` also
  // fires when the menu opens underneath a cursor that is sitting
  // still, which silently moved the highlight off the best match — and
  // Enter would then add whatever happened to be under the pointer.
  const suggestions = $derived.by(() => {
    const q = draft.trim().toUpperCase();
    const taken = new Set(symbols);
    const starts: string[] = [];
    const contains: string[] = [];
    for (const s of universe.list) {
      if (taken.has(s)) continue;
      if (!q || s.startsWith(q)) starts.push(s);
      else if (s.includes(q)) contains.push(s);
      if (starts.length >= 4000) break;
    }
    // Exact match first, then familiar symbols, then shorter tickers —
    // the listed, liquid names are the short ones — then alphabetical.
    const rank = (s: string) =>
      (s === q ? 0 : 1) * 1e6 + (familiar.has(s) ? 0 : 1) * 1e3 + s.length;
    starts.sort((a, b) => rank(a) - rank(b) || a.localeCompare(b));
    return [...starts, ...contains].slice(0, LIMIT);
  });

  $effect(() => {
    void suggestions;
    highlight = 0;
  });

  function normalise(raw: string): string {
    return raw.trim().toUpperCase().replace(/[^A-Z0-9._/^-]/g, "");
  }

  function add(raw: string) {
    const s = normalise(raw);
    if (!s || symbols.includes(s)) return;
    symbols = [...symbols, s];
  }

  function addMany(text: string) {
    for (const tok of text.split(/[\s,;]+/)) add(tok);
  }

  function remove(s: string) {
    symbols = symbols.filter((x) => x !== s);
    inputEl?.focus();
  }

  function commit() {
    const pick = open && suggestions.length && draft.trim() ? suggestions[highlight] : draft;
    add(pick ?? draft);
    draft = "";
    open = true;
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      open = true;
      highlight = Math.min(highlight + 1, suggestions.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      highlight = Math.max(highlight - 1, 0);
    } else if (e.key === "Enter" || e.key === "," || e.key === " " || (e.key === "Tab" && draft.trim())) {
      if (!draft.trim()) return;
      e.preventDefault();
      commit();
    } else if (e.key === "Backspace" && draft === "" && symbols.length) {
      symbols = symbols.slice(0, -1);
    } else if (e.key === "Escape") {
      open = false;
    }
  }

  function onPaste(e: ClipboardEvent) {
    const text = e.clipboardData?.getData("text") ?? "";
    if (/[\s,;]/.test(text.trim())) {
      e.preventDefault();
      addMany(text);
      draft = "";
    }
  }

  function onBlur() {
    // Let a click on a suggestion land first. And check focus really
    // left: a window losing focus and getting it back fires blur then
    // focus, and closing unconditionally here would shut the menu
    // after it had just reopened.
    setTimeout(() => {
      if (document.activeElement === inputEl) return;
      if (draft.trim()) {
        add(draft);
        draft = "";
      }
      open = false;
    }, 140);
  }
</script>

<div class="chips-field" class:focused={open} onclick={() => inputEl?.focus()} role="presentation">
  {#each symbols as s (s)}
    {@const unknown = universe.list.length > 0 && !known.has(s)}
    <span class="chip" class:unknown title={unknown ? `${s} is not listed for this asset class — check the ticker` : s}>
      {#if unknown}<AlertTriangle size={11} />{/if}
      {s}
      <button type="button" class="chip-x" onclick={(e) => { e.stopPropagation(); remove(s); }} aria-label="Remove {s}">
        <X size={11} />
      </button>
    </span>
  {/each}
  <input
    bind:this={inputEl}
    bind:value={draft}
    class="chips-input"
    placeholder={symbols.length ? "" : placeholder}
    onfocus={() => { open = true; void ensureUniverse(); }}
    onblur={onBlur}
    oninput={() => (open = true)}
    onkeydown={onKey}
    onpaste={onPaste}
    aria-label="Add a symbol"
    autocomplete="off"
    spellcheck="false"
  />
  {#if symbols.length > 1}
    <button type="button" class="clear-all" onclick={(e) => { e.stopPropagation(); symbols = []; }}>
      Clear
    </button>
  {/if}

  {#if open && (suggestions.length || universe.loading)}
    <ul class="menu" role="listbox">
      {#if universe.loading}
        <li class="menu-row muted"><Loader2 size={13} class="spin" /> Loading symbols…</li>
      {:else}
        {#each suggestions as s, i (s)}
          <li>
            <button
              type="button"
              class="menu-row"
              class:active={i === highlight}
              role="option"
              aria-selected={i === highlight}
              onmousedown={(e) => { e.preventDefault(); add(s); draft = ""; inputEl?.focus(); }}
              onmousemove={() => (highlight = i)}
            >{s}</button>
          </li>
        {/each}
      {/if}
    </ul>
  {/if}
</div>

<style>
  .chips-field {
    position: relative;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-height: 40px;
    padding: 5px 8px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    cursor: text;
  }
  .chips-field.focused {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-tint);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 4px 2px 8px;
    border-radius: var(--r-pill);
    background: var(--accent-tint);
    color: var(--fg);
    font-family: var(--font-numeric);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
  }
  .chip.unknown { background: var(--warn-tint); color: var(--warn); }
  .chip-x {
    display: inline-flex;
    border: none;
    background: none;
    padding: 2px;
    border-radius: 50%;
    color: inherit;
    opacity: 0.6;
    cursor: pointer;
  }
  .chip-x:hover { opacity: 1; background: var(--surface-3); }
  .chips-input {
    flex: 1;
    min-width: 120px;
    border: none;
    outline: none;
    background: none;
    color: var(--fg);
    font: inherit;
    text-transform: uppercase;
    padding: 4px 2px;
  }
  /* The field draws the focus ring; the global input:focus rule would
     draw a second box inside it. */
  .chips-input:focus { border: none; box-shadow: none; }
  .chips-input::placeholder { text-transform: none; color: var(--fg-subtle); }
  .clear-all {
    border: none;
    background: none;
    color: var(--fg-subtle);
    font-size: var(--text-caption);
    cursor: pointer;
  }
  .clear-all:hover { color: var(--fg); }
  .menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    z-index: 30;
    list-style: none;
    margin: 0;
    padding: 4px;
    max-height: 260px;
    overflow-y: auto;
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-sm);
    box-shadow: var(--shadow-md, 0 8px 24px rgba(0, 0, 0, 0.18));
  }
  .menu-row {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 8px;
    border: none;
    background: none;
    border-radius: 4px;
    color: var(--fg);
    font-family: var(--font-numeric);
    font-size: var(--text-body-sm);
    text-align: left;
    cursor: pointer;
  }
  .menu-row.active { background: var(--accent-tint); }
  .menu-row.muted { color: var(--fg-subtle); cursor: default; }
</style>

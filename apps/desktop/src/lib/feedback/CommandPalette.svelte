<script lang="ts">
  /**
   * Cmd/Ctrl-K palette. Single-search box that fuzzy-matches across:
   *   - saved searches (recently used first)
   *   - datasets (the curated 7 + every registered endpoint)
   *   - cached symbols (stock + option roots)
   *   - schedules
   *   - top-level navigation actions
   *
   * Esc closes; arrows navigate; Enter activates the highlighted row.
   * Designed to be the dominant accelerator for the whole app — every
   * pro tool has one.
   */
  import { onMount, onDestroy } from "svelte";
  import {
    Search,
    Compass,
    Database,
    ListChecks,
    CalendarClock,
    Settings as SettingsIcon,
    Bookmark,
    BarChart2,
    TrendingUp,
    Loader2,
    Tag,
  } from "lucide-svelte";
  import {
    app,
    navigate,
    openDetail,
    openComposer,
    DATASETS,
    type DatasetMeta,
    datasetFromCatalogue,
    datasetById,
    log,
  } from "$lib/stores/app.svelte";
  import { listSavedSearches, touchSavedSearch, type SavedSearch } from "$lib/persistence/savedSearches";

  type Item =
    | { kind: "nav"; label: string; view: "browse" | "library" | "queue" | "schedules" | "settings"; hint?: string; icon: typeof Search }
    | { kind: "dataset"; label: string; dataset: DatasetMeta; hint: string; icon: typeof Search }
    | { kind: "symbol"; label: string; symbol: string; hint: string; icon: typeof Search }
    | { kind: "saved"; label: string; saved: SavedSearch; hint: string; icon: typeof Search };

  const NAV: Item[] = [
    { kind: "nav", label: "Browse",    view: "browse",    icon: Compass,         hint: "Browse all datasets" },
    { kind: "nav", label: "Library",   view: "library",   icon: Database,        hint: "Datasets on disk" },
    { kind: "nav", label: "Queue",     view: "queue",     icon: ListChecks,      hint: "Active downloads" },
    { kind: "nav", label: "Schedules", view: "schedules", icon: CalendarClock,   hint: "Recurring downloads" },
    { kind: "nav", label: "Settings",  view: "settings",  icon: SettingsIcon,    hint: "Account & storage" },
  ];

  let query = $state("");
  let highlight = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  // Every dataset the account can reach, not the seven-entry static
  // mirror this used to search — that covered a ninth of the catalogue
  // and made the palette useless for the rest.
  const datasetItems = $derived<Item[]>(
    app.catalogue.map((e) => {
      const d = datasetFromCatalogue(e);
      return {
        kind: "dataset" as const,
        label: d.title,
        dataset: d,
        hint: `${d.assetClass.toUpperCase()} · ${e.name}`,
        icon: d.assetClass === "option" ? BarChart2 : TrendingUp,
      };
    }),
  );

  const symbolItems = $derived<Item[]>(
    [
      ...app.symbols.stockSymbols.map((s) => ({
        kind: "symbol" as const,
        label: s,
        symbol: s,
        hint: "Stock",
        icon: TrendingUp,
      })),
      ...app.symbols.optionRoots.map((s) => ({
        kind: "symbol" as const,
        label: s,
        symbol: s,
        hint: "Option root",
        icon: BarChart2,
      })),
    ].slice(0, 80),
  );

  const savedItems = $derived<Item[]>(
    listSavedSearches().map((s) => ({
      kind: "saved" as const,
      label: s.name,
      saved: s,
      hint: `${s.symbols.length === 1 ? s.symbols[0] : `${s.symbols.length} symbols`} · ${s.kind}`,
      icon: Bookmark,
    })),
  );

  const all = $derived<Item[]>([...NAV, ...savedItems, ...datasetItems, ...symbolItems]);

  // Tiny fuzzy: case-insensitive substring match, with a starts-with
  // boost. Keeps things fast for 200+ rows without a dep.
  function score(label: string, q: string): number {
    if (!q) return 1;
    const l = label.toLowerCase();
    const idx = l.indexOf(q.toLowerCase());
    if (idx < 0) return 0;
    return idx === 0 ? 100 : 50 - idx;
  }

  const filtered = $derived.by(() => {
    const q = query.trim();
    if (!q) return all.slice(0, 25);
    return all
      .map((item) => ({ item, s: score(item.label, q) }))
      .filter((x) => x.s > 0)
      .sort((a, b) => b.s - a.s)
      .slice(0, 25)
      .map((x) => x.item);
  });

  function close() { app.cmdkOpen2 = false; }

  function pick(item: Item) {
    if (item.kind === "nav") {
      navigate(item.view);
    } else if (item.kind === "dataset") {
      openDetail(item.dataset);
    } else if (item.kind === "symbol") {
      // Drop into the composer pre-populated with this symbol.
      openComposer(datasetById("stock_history_trade_quote") ?? DATASETS[0]);
      app.composer.symbol = item.symbol;
    } else if (item.kind === "saved") {
      touchSavedSearch(item.saved.id);
      // Resolve against the live catalogue. Falling back to the first
      // static entry silently opened the composer on the wrong dataset
      // for any saved search outside the original seven.
      const ds = datasetById(item.saved.kind);
      if (!ds) {
        log("error", `Saved search "${item.saved.name}" names an unknown dataset (${item.saved.kind})`);
        close();
        return;
      }
      openComposer(ds);
      app.composer.symbol = item.saved.symbols.join(", ");
      app.composer.start = item.saved.start ?? "";
      app.composer.end = item.saved.end ?? "";
      app.composer.format = item.saved.format as typeof app.composer.format;
    }
    close();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") { close(); return; }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      highlight = Math.min(highlight + 1, filtered.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      highlight = Math.max(highlight - 1, 0);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const it = filtered[highlight];
      if (it) pick(it);
    }
  }

  // Reset highlight when query changes.
  $effect(() => { void query; highlight = 0; });

  // Focus input when opened.
  $effect(() => {
    if (app.cmdkOpen2 && inputEl) {
      inputEl.focus();
      query = "";
    }
  });

  // Global Cmd/Ctrl-K opens it.
  function globalKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      app.cmdkOpen2 = !app.cmdkOpen2;
    }
  }
  onMount(() => window.addEventListener("keydown", globalKey));
  onDestroy(() => window.removeEventListener("keydown", globalKey));
</script>

{#if app.cmdkOpen2}
  <div class="backdrop" onclick={close} role="presentation">
    <div
      class="palette"
      onclick={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
      tabindex="-1"
      onkeydown={onKey}
    >
      <header class="head">
        <Search size={16} class="head-icon" />
        <input
          bind:this={inputEl}
          bind:value={query}
          class="input"
          placeholder="Search datasets, symbols, saved searches…"
          autocomplete="off"
          spellcheck="false"
          role="combobox"
          aria-expanded="true"
          aria-controls="cmdk-list"
          aria-activedescendant={filtered.length ? `cmdk-option-${highlight}` : undefined}
        />
        <span class="hint text-caption">Esc</span>
      </header>

      <ul id="cmdk-list" class="list" role="listbox" aria-label="Results">
        {#if filtered.length === 0}
          <li class="empty fg-muted">
            No matches. {#if !app.symbols.loadedAt}
              <span class="fg-subtle">(Symbol cache still warming up.)</span>
            {/if}
          </li>
        {/if}
        {#each filtered as item, i (item.kind + ":" + item.label)}
          {@const Icon = item.icon}
          <!-- Keyboard lives on the input via `aria-activedescendant`,
               which is the combobox pattern; the pointer handlers here
               are the mouse half of the same control. -->
          <li
            id="cmdk-option-{i}"
            class="row"
            class:active={i === highlight}
            role="option"
            aria-selected={i === highlight}
            onmouseenter={() => (highlight = i)}
            onclick={() => pick(item)}
            onkeydown={(e) => e.key === "Enter" && pick(item)}
          >
            <span class="row-icon"><Icon size={14} /></span>
            <span class="row-label">{item.label}</span>
            <span class="row-hint text-caption">
              {item.hint ?? ""}
            </span>
            <span class="row-kind text-caption">
              {item.kind}
            </span>
          </li>
        {/each}
      </ul>

      <footer class="foot text-caption fg-muted">
        <span>↑↓ navigate · ↵ activate · Esc close</span>
        {#if app.symbols.loading}
          <span class="warming"><Loader2 size={11} class="spin" /> Warming caches…</span>
        {:else}
          <span class="counts tabnum">
            <Tag size={10} />
            {app.symbols.stockSymbols.length} stocks · {app.symbols.optionRoots.length} option roots
          </span>
        {/if}
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed; inset: 0;
    background: var(--scrim);
    backdrop-filter: blur(6px);
    display: flex; align-items: flex-start; justify-content: center;
    padding-top: 12vh;
    z-index: 200;
  }
  .palette {
    width: 640px;
    max-width: calc(100vw - var(--sp-8));
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-modal);
    display: flex; flex-direction: column;
    overflow: hidden;
    animation: cmdk-pop var(--dur-base) var(--ease-decel);
  }
  @keyframes cmdk-pop {
    from { transform: translateY(-8px); opacity: 0; }
    to   { transform: translateY(0);    opacity: 1; }
  }
  .head {
    display: flex; align-items: center; gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--border);
  }
  .head :global(.head-icon) { color: var(--fg-muted); }
  .input {
    flex: 1;
    background: transparent;
    border: 0;
    color: var(--fg);
    font-size: var(--text-body);
    outline: none;
    font-family: var(--font-ui);
  }
  .input::placeholder { color: var(--fg-subtle); }
  .hint {
    background: var(--surface-3);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 1px 6px;
    color: var(--fg-muted);
  }

  .list {
    list-style: none;
    margin: 0;
    padding: var(--sp-1);
    max-height: 50vh;
    overflow-y: auto;
  }
  .row {
    display: grid;
    grid-template-columns: 20px 1fr auto auto;
    gap: var(--sp-3);
    align-items: center;
    padding: 8px var(--sp-3);
    cursor: pointer;
    border-radius: var(--r-sm);
  }
  .row:hover, .row.active {
    background: var(--accent-tint);
  }
  .row-icon { color: var(--fg-muted); display: inline-flex; }
  .row.active .row-icon { color: var(--accent-hi); }
  .row-label { color: var(--fg); font-size: var(--text-body-sm); }
  .row-hint { color: var(--fg-muted); }
  .row-kind {
    color: var(--fg-subtle);
    font-family: var(--font-mono);
    text-transform: lowercase;
    letter-spacing: 0;
  }
  .empty { padding: var(--sp-4); text-align: center; }

  .foot {
    border-top: 1px solid var(--border);
    padding: 6px var(--sp-3);
    display: flex; justify-content: space-between; align-items: center;
    background: var(--surface-1);
  }
  .warming { display: inline-flex; align-items: center; gap: 4px; }
  .counts { display: inline-flex; align-items: center; gap: 4px; }
</style>

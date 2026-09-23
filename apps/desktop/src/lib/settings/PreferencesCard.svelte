<script lang="ts">
  /**
   * How the user works, so the app stops asking.
   *
   * Every field here either pre-fills something the user could set per
   * download, or tunes behaviour within limits the account imposes. None
   * of it blocks a choice: Browse still lets you pick any format or
   * dataset, it just starts where you usually end up.
   */
  import { Plus, Trash2, ListPlus } from "lucide-svelte";
  import { app } from "$lib/stores/app.svelte";
  import type { Preferences } from "$lib/api";

  const prefs = $derived(app.settings.preferences as Preferences);

  const FORMATS = [
    { id: "parquet", label: "Parquet" },
    { id: "csv", label: "CSV" },
    { id: "jsonl", label: "JSON Lines" },
    { id: "json", label: "JSON" },
  ] as const;

  const CLASSES = [
    { id: "stock", label: "Stocks" },
    { id: "option", label: "Options" },
    { id: "index", label: "Indices" },
    { id: "rate", label: "Rates" },
  ] as const;

  const RANGES = [1, 2, 3, 5, 10];

  /** Datasets in the chosen class that queue as downloads — the list
   *  endpoints run once and never become a default. */
  const datasetChoices = $derived(
    app.catalogue
      .filter((e) => {
        const cls =
          e.name.startsWith("option_") ? "option"
          : e.name.startsWith("index_") ? "index"
          : e.name.startsWith("interest_rate") ? "rate"
          : e.name.startsWith("stock_") ? "stock"
          : null;
        return (
          cls === prefs.default_asset_class &&
          !e.subcategory.startsWith("list") &&
          !e.subcategory.startsWith("snapshot")
        );
      })
      .sort((a, b) => (a.summary || a.name).localeCompare(b.summary || b.name)),
  );

  // A default dataset from another class would be silently ignored by
  // Browse; clear it rather than leave a setting that does nothing.
  $effect(() => {
    const ds = prefs.default_dataset;
    if (ds && !datasetChoices.some((e) => e.name === ds)) {
      prefs.default_dataset = null;
    }
  });

  const budget = $derived(app.tierStatus?.in_flight_budget ?? 0);
  const capped = $derived(prefs.max_concurrency !== null);

  function toggleCap(on: boolean) {
    prefs.max_concurrency = on ? Math.max(1, Math.min(budget || 4, 4)) : null;
  }

  // ── Watchlists ───────────────────────────────────────────────
  /** Accept whatever the user pastes: commas, spaces, newlines, tabs. */
  function parseSymbols(raw: string): string[] {
    const seen = new Set<string>();
    for (const tok of raw.toUpperCase().split(/[\s,;]+/)) {
      const t = tok.trim();
      if (t) seen.add(t);
    }
    return [...seen];
  }

  function addWatchlist() {
    prefs.watchlists = [
      ...prefs.watchlists,
      { name: `Watchlist ${prefs.watchlists.length + 1}`, symbols: [] },
    ];
  }

  function removeWatchlist(i: number) {
    prefs.watchlists = prefs.watchlists.filter((_, j) => j !== i);
  }

  function setSymbols(i: number, raw: string) {
    const next = [...prefs.watchlists];
    next[i] = { ...next[i], symbols: parseSymbols(raw) };
    prefs.watchlists = next;
  }
</script>

<section class="settings-card prefs">
  <h2 class="card-heading">Preferences</h2>

  <div class="group">
    <h3 class="group-title text-caption">Start new downloads with</h3>
    <div class="row-4">
      <label class="field-stack">
        <span class="text-caption">Format</span>
        <select class="field-input" bind:value={prefs.default_format}>
          {#each FORMATS as f (f.id)}<option value={f.id}>{f.label}</option>{/each}
        </select>
      </label>
      <label class="field-stack">
        <span class="text-caption">Asset class</span>
        <select class="field-input" bind:value={prefs.default_asset_class}>
          {#each CLASSES as c (c.id)}<option value={c.id}>{c.label}</option>{/each}
        </select>
      </label>
      <label class="field-stack">
        <span class="text-caption">Dataset</span>
        <select class="field-input" bind:value={prefs.default_dataset}>
          <option value={null}>Let me choose</option>
          {#each datasetChoices as e (e.name)}
            <option value={e.name}>{e.summary || e.name} · {e.subcategory.replace(/_/g, " ")}</option>
          {/each}
        </select>
      </label>
      <label class="field-stack">
        <span class="text-caption">Range</span>
        <select class="field-input" bind:value={prefs.default_range_years}>
          {#each RANGES as y (y)}<option value={y}>{y} year{y === 1 ? "" : "s"}</option>{/each}
        </select>
      </label>
    </div>
  </div>

  <div class="group">
    <h3 class="group-title text-caption">Downloads</h3>

    <label class="toggle-row">
      <input type="checkbox" checked={capped} onchange={(e) => toggleCap(e.currentTarget.checked)} />
      <span class="toggle-text">
        <span>Limit concurrent downloads</span>
        <span class="hint fg-muted">
          Your plan allows {budget || "—"} at once, across the whole account. Leave headroom if
          you run your own scripts against it.
        </span>
      </span>
      {#if capped}
        <input
          class="field-input cap-input tabnum"
          type="number"
          min="1"
          max={budget || 64}
          bind:value={prefs.max_concurrency}
          aria-label="Maximum concurrent downloads"
        />
      {/if}
    </label>

    <label class="toggle-row">
      <input type="checkbox" bind:checked={prefs.split_failed_windows} />
      <span class="toggle-text">
        <span>Retry a failed multi-day download in halves</span>
        <span class="hint fg-muted">
          Keeps what succeeded and narrows in on the stretch that failed, instead of refetching
          the whole window.
        </span>
      </span>
    </label>

    <label class="toggle-row">
      <input type="checkbox" bind:checked={prefs.notify_on_complete} />
      <span class="toggle-text">
        <span>Notify when a run finishes</span>
        <span class="hint fg-muted">A system notification with what was downloaded and what failed.</span>
      </span>
    </label>
  </div>

  <div class="group">
    <div class="group-head">
      <h3 class="group-title text-caption">Watchlists</h3>
      <button type="button" class="btn btn-ghost btn-sm" onclick={addWatchlist}>
        <Plus size={13} /> New watchlist
      </button>
    </div>
    {#if prefs.watchlists.length === 0}
      <p class="empty hint fg-muted">
        <ListPlus size={14} />
        Named symbol lists you can pick in Browse, instead of typing the same tickers again.
      </p>
    {:else}
      <ul class="watchlists">
        {#each prefs.watchlists as w, i (i)}
          <li class="watchlist">
            <input class="field-input wl-name" bind:value={w.name} aria-label="Watchlist name" />
            <textarea
              class="field-input wl-symbols tabnum"
              rows="2"
              value={w.symbols.join(", ")}
              onchange={(e) => setSymbols(i, e.currentTarget.value)}
              placeholder="AAPL, MSFT, NVDA — paste with commas, spaces or new lines"
              aria-label="Symbols in {w.name}"
            ></textarea>
            <span class="wl-count text-caption tabnum">{w.symbols.length}</span>
            <button
              type="button"
              class="btn-icon"
              onclick={() => removeWatchlist(i)}
              aria-label="Delete {w.name}"
            >
              <Trash2 size={14} />
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</section>

<style>
  .prefs { margin-top: var(--sp-4); }
  .group { display: flex; flex-direction: column; gap: var(--sp-3); }
  .group + .group {
    border-top: 1px solid var(--border);
    padding-top: var(--sp-4);
  }
  .group-head { display: flex; align-items: center; justify-content: space-between; }
  .group-title { margin: 0; color: var(--fg-muted); }
  .row-4 {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--sp-3);
  }
  @media (max-width: 1000px) { .row-4 { grid-template-columns: 1fr 1fr; } }

  .toggle-row {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: start;
    gap: var(--sp-3);
    cursor: pointer;
  }
  .toggle-row input[type="checkbox"] { margin-top: 3px; }
  .toggle-text { display: flex; flex-direction: column; gap: 2px; }
  .cap-input { width: 72px; }

  .empty {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin: 0;
  }
  .watchlists { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--sp-2); }
  .watchlist {
    display: grid;
    grid-template-columns: 180px 1fr auto auto;
    gap: var(--sp-2);
    align-items: start;
  }
  .wl-symbols { resize: vertical; min-height: 34px; font-family: var(--font-numeric); }
  .wl-count { color: var(--fg-subtle); padding-top: 8px; min-width: 2ch; text-align: right; }
</style>

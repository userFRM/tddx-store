<script lang="ts">
  /**
   * KindGrid — dataset catalogue browser.
   *
   * Two-axis surface:
   *
   *   ┌──────────────────────────────────────────────────────────┐
   *   │  [All 9]  [⏱ History 4]  [⏰ At time 1]  [📅 EOD 1]  …  │  ← chips
   *   ├──────────────────────────────────────────────────────────┤
   *   │  HISTORY                                            (4)  │
   *   │  • Trade        · Per-tick    STOCK  STANDARD   7p  ▼   │
   *   │  • Quote        · NBBO        STOCK  STANDARD   8p  ▼   │
   *   │  …                                                       │
   *   │  AT TIME                                            (1)  │
   *   │  • Trade        · At time     STOCK  STANDARD   5p  ▼   │
   *   └──────────────────────────────────────────────────────────┘
   *
   * The user grouped-by-tag layout collapsed History+At time into one
   * column and produced three rows labelled "Trade" that all looked
   * identical. Grouping by `subcategory` makes the access pattern
   * (point-in-time vs full-day vs EOD vs list) the primary axis, with
   * a chip strip that doubles as a filter when the user only wants to
   * see one family.
   *
   * Subcategory metadata lives in the YAML — we don't hardcode the
   * list. Subcategories that surface in `filtered` are the only ones
   * rendered, so adding a new subcategory upstream automatically gets
   * a chip + group without a code change.
   */
  import { Loader2, Clock, Timer, Calendar, List, Sigma, Compass } from "lucide-svelte";
  import { app, tierForKind, type AssetClass } from "$lib/stores/app.svelte";
  import { TIER_RANK, type CatalogueEntry } from "$lib/api";
  import DatasetRow from "./DatasetRow.svelte";

  // The store is a downloader, not a terminal — snapshot endpoints
  // (current-tick / NBBO) don't make sense as bulk-downloadable
  // datasets. Hide them from the catalogue here.
  const HIDDEN_SUBCATEGORIES = new Set(["snapshot", "snapshot_greeks"]);

  let {
    assetClass,
    selectedKindId = $bindable(""),
  }: {
    assetClass: AssetClass;
    selectedKindId: string;
  } = $props();

  // ── Subcategory metadata ─────────────────────────────────────
  // Display labels + icons per known subcategory. Anything not in
  // the table falls back to its raw yaml string + a default icon.
  // Order here defines the chip strip ordering.
  type SubcatMeta = { key: string; label: string; icon: typeof Clock };
  const SUBCAT_META: SubcatMeta[] = [
    { key: "history",                 label: "History",      icon: Clock },
    { key: "history_eod",             label: "EOD",          icon: Calendar },
    { key: "history_greeks",          label: "Greeks",       icon: Sigma },
    { key: "history_greeks_eod",      label: "Greeks · EOD", icon: Sigma },
    { key: "at_time",                 label: "At time",      icon: Timer },
    { key: "list",                    label: "Lists",        icon: List },
  ];
  function metaFor(key: string): SubcatMeta {
    return (
      SUBCAT_META.find((m) => m.key === key) ?? {
        key,
        label: key.replace(/_/g, " "),
        icon: Compass,
      }
    );
  }

  // ── Catalogue filtering ───────────────────────────────────────
  const filtered = $derived.by((): CatalogueEntry[] =>
    app.catalogue.filter(
      (e) =>
        e.category === assetClass && !HIDDEN_SUBCATEGORIES.has(e.subcategory),
    ),
  );

  // ── Active subcategory chip ──────────────────────────────────
  // `""` = All. Toggling a chip filters the visible list to that
  // subcategory. Reset to All whenever the asset class changes.
  let activeSubcat = $state<string>("");
  $effect(() => {
    // Side-effect ONLY on assetClass — reset chip, leave selection
    // alone (auto-select below handles cross-class selection drift).
    assetClass; // touch
    activeSubcat = "";
  });

  // ── Group by subcategory, ordered by SUBCAT_META, tail sorted by
  //    required tier ascending then yaml order. ────────────────
  type Group = { key: string; meta: SubcatMeta; entries: CatalogueEntry[] };
  const grouped = $derived.by((): Group[] => {
    const buckets = new Map<string, CatalogueEntry[]>();
    for (const e of filtered) {
      const k = e.subcategory || "other";
      if (!buckets.has(k)) buckets.set(k, []);
      buckets.get(k)!.push(e);
    }
    const orderedKeys: string[] = [];
    // Seed with the meta-known order, then append any leftover keys
    // discovered in the yaml so new subcategories don't disappear.
    for (const m of SUBCAT_META) if (buckets.has(m.key)) orderedKeys.push(m.key);
    for (const k of buckets.keys()) if (!orderedKeys.includes(k)) orderedKeys.push(k);
    return orderedKeys.map((k) => ({
      key: k,
      meta: metaFor(k),
      entries: buckets.get(k)!.slice().sort((a, b) => {
        const ra = TIER_RANK[a.min_tier ?? "Unknown"] ?? -1;
        const rb = TIER_RANK[b.min_tier ?? "Unknown"] ?? -1;
        return ra - rb;
      }),
    }));
  });

  /** Groups passing the active-chip filter (`""` = All). */
  const visibleGroups = $derived.by(() =>
    activeSubcat === "" ? grouped : grouped.filter((g) => g.key === activeSubcat),
  );

  // ── Auto-select first entry when asset class changes and the
  //    current selection no longer exists in the filtered set.
  const filteredIds = $derived(filtered.map((e) => e.name));
  function handleSelect(name: string) {
    selectedKindId = name;
  }
  $effect(() => {
    if (filteredIds.length > 0 && !filteredIds.includes(selectedKindId)) {
      selectedKindId = filteredIds[0];
    }
  });
</script>

<div class="catalogue-list" role="table" aria-label="Dataset catalogue">
  {#if app.catalogueLoading}
    <div class="state-overlay loading">
      <Loader2 size={16} strokeWidth={1.75} class="spin" aria-hidden="true" />
      <span class="text-body-sm fg-muted">Loading catalogue…</span>
    </div>

  {:else if filtered.length === 0}
    <div class="state-overlay empty">
      <p class="text-body-sm fg-muted">No datasets found for this asset class.</p>
    </div>

  {:else}
    <!-- ── Subcategory chip strip ─────────────────────────────── -->
    <div class="chip-strip" role="tablist" aria-label="Filter by access pattern">
      <button
        type="button"
        role="tab"
        class="chip"
        class:active={activeSubcat === ""}
        aria-selected={activeSubcat === ""}
        onclick={() => (activeSubcat = "")}
      >
        <span class="chip-label">All</span>
        <span class="chip-count tabnum">{filtered.length}</span>
      </button>
      {#each grouped as g (g.key)}
        {@const Icon = g.meta.icon}
        <button
          type="button"
          role="tab"
          class="chip"
          class:active={activeSubcat === g.key}
          aria-selected={activeSubcat === g.key}
          onclick={() => (activeSubcat = g.key)}
        >
          <Icon size={12} strokeWidth={1.75} aria-hidden="true" />
          <span class="chip-label">{g.meta.label}</span>
          <span class="chip-count tabnum">{g.entries.length}</span>
        </button>
      {/each}
    </div>

    <!-- ── Grouped rows ──────────────────────────────────────── -->
    {#each visibleGroups as group (group.key)}
      {@const Icon = group.meta.icon}
      <div class="group-header" role="rowgroup" aria-label="{group.meta.label} group">
        <Icon size={13} strokeWidth={1.75} aria-hidden="true" />
        <span class="group-label">{group.meta.label}</span>
        <span class="group-count tabnum" aria-label="{group.entries.length} datasets">
          {group.entries.length}
        </span>
        <div class="group-rule" aria-hidden="true"></div>
      </div>

      <div class="group-rows" role="rowgroup">
        {#each group.entries as entry (entry.name)}
          <DatasetRow
            {entry}
            selected={selectedKindId === entry.name}
            onselect={handleSelect}
          />
        {/each}
      </div>
    {/each}
  {/if}
</div>

<style>
  .catalogue-list {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    background: var(--surface-1);
    overflow: hidden;
  }

  .state-overlay {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-6) var(--sp-5);
    color: var(--fg-muted);
  }

  /* ── Chip strip ─────────────────────────────────────────────── */
  .chip-strip {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 var(--sp-3);
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    background: var(--surface-1);
    color: var(--fg-muted);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }
  .chip:hover { color: var(--fg); border-color: var(--border-strong); }
  .chip.active {
    background: var(--accent-tint);
    border-color: var(--accent);
    color: var(--accent-hi);
  }
  .chip-label { letter-spacing: 0.01em; }
  .chip-count {
    font-size: var(--text-caption);
    color: var(--fg-subtle);
    background: var(--surface-3);
    border-radius: var(--r-pill);
    padding: 1px 6px;
  }
  .chip.active .chip-count {
    background: rgba(255, 255, 255, 0.18);
    color: inherit;
  }

  /* ── Group header ───────────────────────────────────────────── */
  .group-header {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-4);
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    color: var(--fg-subtle);
  }
  .group-rows + .group-header { border-top: 1px solid var(--border); }
  .group-label {
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    white-space: nowrap;
  }
  .group-count {
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    padding: 1px 6px;
  }
  .group-rule {
    flex: 1;
    height: 1px;
    background: var(--border);
  }

  .group-rows {
    display: flex;
    flex-direction: column;
  }
</style>

<script lang="ts">
  /**
   * Auto-generated Browse shelves for every registered thetadatadx
   * endpoint, grouped by category (stock / option / index / rate /
   * calendar) and subcategory (history / snapshot / list / at_time /
   * history_greeks). Each card opens the dataset detail page with the
   * endpoint's metadata.
   *
   * Backed by the `endpoints_list` Tauri command — works only inside
   * the desktop runtime; in the browser preview we render a friendly
   * empty state.
   */
  import { onMount } from "svelte";
  import { Compass, Loader2 } from "lucide-svelte";
  import EndpointCard from "$lib/catalogue/EndpointCard.svelte";
  import { api, type EndpointInfo, TAURI_AVAILABLE } from "$lib/api";
  import { log } from "$lib/stores/app.svelte";

  let endpoints = $state<EndpointInfo[]>([]);
  let loading = $state(false);
  let err = $state<string | null>(null);

  onMount(async () => {
    if (!TAURI_AVAILABLE) return;
    loading = true;
    try {
      endpoints = await api.endpointsList();
      log("info", `Loaded ${endpoints.length} endpoints from registry`);
    } catch (e: unknown) {
      err = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  });

  // Snapshot endpoints (current-tick / NBBO state) are intentionally
  // hidden from the catalogue — TdDx Store is a bulk historical
  // downloader, not a real-time terminal. Users who need snapshot
  // calls hit them directly via `endpoint_invoke` from the runner
  // pane.
  const HIDDEN_SUBCATEGORIES = new Set(["snapshot", "snapshot_greeks"]);

  // Group by category > subcategory.
  const byCategory = $derived.by(() => {
    const map = new Map<string, Map<string, EndpointInfo[]>>();
    for (const e of endpoints) {
      if (HIDDEN_SUBCATEGORIES.has(e.subcategory)) continue;
      let cat = map.get(e.category);
      if (!cat) { cat = new Map(); map.set(e.category, cat); }
      let sub = cat.get(e.subcategory);
      if (!sub) { sub = []; cat.set(e.subcategory, sub); }
      sub.push(e);
    }
    return map;
  });

  const CATEGORY_ORDER = ["stock", "option", "index", "rate", "calendar"];
  const SUBCAT_LABELS: Record<string, string> = {
    history: "History",
    list: "List",
    at_time: "At time",
    history_greeks: "History · Greeks",
  };
</script>

{#if !TAURI_AVAILABLE}
  <!-- nothing — Browse falls back to the curated tick shelves -->
{:else if loading}
  <div class="state">
    <Loader2 class="spin" size={20} />
    <span class="text-body fg-muted">Loading endpoints from registry…</span>
  </div>
{:else if err}
  <div class="state error">
    <span>Failed to load endpoint registry: {err}</span>
  </div>
{:else}
  {#each CATEGORY_ORDER as cat}
    {#if byCategory.has(cat)}
      {@const subs = byCategory.get(cat)!}
      <section class="cat">
        <header class="cat-header">
          <h2 class="cat-title">
            {cat === "stock" ? "Equities"
             : cat === "option" ? "Options"
             : cat === "index" ? "Indices"
             : cat === "rate" ? "Rates"
             : "Calendar"}
          </h2>
          <span class="cat-count tabnum text-caption">
            {Array.from(subs.values()).reduce((n, v) => n + v.length, 0)} endpoints
          </span>
        </header>
        {#each [...subs.entries()] as [subKey, list]}
          <section class="sub">
            <span class="sub-label text-caption">
              {SUBCAT_LABELS[subKey] ?? subKey}
            </span>
            <div class="grid">
              {#each list as ep (ep.name)}
                <EndpointCard endpoint={ep} />
              {/each}
            </div>
          </section>
        {/each}
      </section>
    {/if}
  {/each}
{/if}

<style>
  .state {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-6);
    border: 1px dashed var(--border);
    border-radius: var(--r-md);
    background: var(--surface-1);
  }
  .state.error { color: var(--bad); border-color: rgba(255,126,126,0.3); }

  .cat {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
  }
  .cat-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    border-bottom: 1px solid var(--border);
    padding-bottom: var(--sp-2);
  }
  .cat-title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    letter-spacing: -0.015em;
    margin: 0;
  }
  .cat-count { color: var(--fg-muted); }

  .sub {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .sub-label { color: var(--fg-muted); }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: var(--sp-3);
  }

  :global(.spin) { animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>

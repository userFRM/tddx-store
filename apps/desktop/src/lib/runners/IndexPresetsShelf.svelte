<script lang="ts">
  /**
   * Browse shelf showing index ecosystems (S&P 500, NDX, Sp400, …)
   * sourced from indexkit. Clicking a card opens a bulk-queue dialog
   * that resolves constituents and queues N tasks per (kind, symbol).
   */
  import { onMount } from "svelte";
  import { Layers, Loader2, ChevronRight } from "lucide-svelte";
  import { api, type IndexPresetView, TAURI_AVAILABLE } from "$lib/api";
  import { app, openIndexPreset, log } from "$lib/stores/app.svelte";

  let presets = $state<IndexPresetView[]>([]);
  let loading = $state(false);

  onMount(async () => {
    if (!TAURI_AVAILABLE) return;
    loading = true;
    try {
      presets = await api.indexPresets();
    } catch (e: unknown) {
      log("warn", `indexkit presets unavailable: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      loading = false;
    }
  });
</script>

<section class="shelf">
  <header>
    <h2 class="title">
      <Layers size={16} />
      Index ecosystems
    </h2>
    <span class="count text-caption">
      {#if loading}<Loader2 class="spin" size={12} />Loading…
      {:else}{presets.length} indices · indexkit
      {/if}
    </span>
  </header>

  {#if presets.length === 0 && !loading}
    <div class="empty text-body-sm fg-muted">
      Connect to ThetaData &amp; rebuild with the <code>presets</code> feature
      to enable index constituent bulk-queue.
    </div>
  {:else}
    <div class="grid">
      {#each presets as p (p.id)}
        <article class="card" onclick={() => openIndexPreset(p)} role="button" tabindex="0"
                 onkeydown={(e) => e.key === "Enter" && openIndexPreset(p)}>
          <div class="head">
            <span class="id-pill text-mono">{p.id.toUpperCase()}</span>
          </div>
          <h3 class="t">{p.name}</h3>
          <p class="desc">{p.description}</p>
          <div class="foot">
            <span class="hint text-caption">Bulk-queue all constituents</span>
            <ChevronRight size={14} class="chev" />
          </div>
        </article>
      {/each}
    </div>
  {/if}
</section>

<style>
  .shelf { display: flex; flex-direction: column; gap: var(--sp-3); }
  header {
    display: flex; justify-content: space-between; align-items: baseline;
    border-bottom: 1px solid var(--border);
    padding-bottom: var(--sp-2);
  }
  .title {
    display: inline-flex; align-items: center; gap: 8px;
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    margin: 0;
  }
  .count {
    color: var(--fg-muted);
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .empty {
    padding: var(--sp-5);
    border: 1px dashed var(--border);
    border-radius: var(--r-md);
    background: var(--surface-1);
  }
  .empty code {
    font-family: var(--font-mono);
    background: var(--surface-3);
    padding: 0 4px;
    border-radius: 3px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: var(--sp-3);
  }
  .card {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    padding: var(--sp-3) var(--sp-4);
    height: 152px;
    display: grid;
    grid-template-rows: auto auto 1fr auto;
    gap: var(--sp-2);
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-standard),
                border-color var(--dur-fast) var(--ease-standard),
                transform var(--dur-fast) var(--ease-standard);
    outline: none;
  }
  .card:hover {
    background: var(--surface-2);
    border-color: var(--border-strong);
    transform: translateY(-1px);
  }
  .head { display: flex; }
  .id-pill {
    font-size: 11px;
    background: var(--accent-tint);
    color: var(--accent-hi);
    padding: 2px 8px;
    border-radius: var(--r-pill);
    font-weight: var(--weight-semi);
    letter-spacing: 0.04em;
  }
  .t { font-size: var(--text-body); font-weight: var(--weight-semi); margin: 0; }
  .desc {
    font-size: var(--text-body-sm); color: var(--fg-muted);
    margin: 0; line-height: 1.4;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .foot {
    display: flex; justify-content: space-between; align-items: center;
    border-top: 1px solid var(--border);
    padding-top: 6px;
  }
  .hint { color: var(--fg-subtle); }
  :global(.chev) { color: var(--fg-subtle); }
  .card:hover :global(.chev) { color: var(--accent-hi); }
  :global(.spin) { animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>

<script lang="ts">
  import { ChevronRight } from "lucide-svelte";
  import DatasetCard from "$lib/catalogue/DatasetCard.svelte";
  import type { DatasetMeta } from "$lib/stores/app.svelte";

  let {
    title,
    datasets,
    placeholder = false,
  }: {
    title: string;
    datasets: DatasetMeta[];
    placeholder?: boolean;
  } = $props();

  let scrollEl = $state<HTMLElement | null>(null);

  function scrollRight() {
    scrollEl?.scrollBy({ left: 560, behavior: "smooth" });
  }
</script>

<section class="shelf">
  <header class="shelf-header">
    <h2 class="shelf-title">{title}</h2>
    {#if !placeholder && datasets.length > 3}
      <button class="see-all" onclick={scrollRight} aria-label="Scroll {title} shelf right">
        See all <ChevronRight size={14} strokeWidth={1.75} />
      </button>
    {/if}
  </header>

  <div class="shelf-track" bind:this={scrollEl} role="list">
    {#if placeholder}
      <div class="placeholder-card">
        <div class="placeholder-icon">
          <svg width="32" height="32" viewBox="0 0 32 32" fill="none" aria-hidden="true">
            <rect x="4" y="4" width="24" height="24" rx="6" stroke="var(--border-strong)" stroke-width="1.5" stroke-dasharray="4 3" />
            <path d="M12 16h8M16 12v8" stroke="var(--fg-subtle)" stroke-width="1.5" stroke-linecap="round" />
          </svg>
        </div>
        <p class="placeholder-label">Index Ecosystems</p>
        <p class="placeholder-sub">S&amp;P 500 · NDX · Sp400 · Sp600 · DJI · RUT</p>
        <span class="coming-soon">Coming soon</span>
      </div>
    {:else}
      {#each datasets as dataset (dataset.id)}
        <div role="listitem">
          <DatasetCard {dataset} />
        </div>
      {/each}
    {/if}
  </div>
</section>

<style>
  .shelf {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }

  .shelf-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 var(--sp-1);
  }

  .shelf-title {
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--fg-muted);
  }

  .see-all {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    letter-spacing: 0.03em;
    text-transform: uppercase;
    color: var(--accent);
    background: none;
    border: none;
    cursor: pointer;
    padding: 2px var(--sp-1);
    border-radius: var(--r-sm);
    transition: color var(--dur-fast) var(--ease-standard);
    outline: none;
  }
  .see-all:hover { color: var(--accent-hi); }
  .see-all:focus-visible { box-shadow: var(--shadow-glow-accent); }

  .shelf-track {
    display: flex;
    gap: var(--sp-3);
    overflow-x: auto;
    overflow-y: visible;
    padding-bottom: var(--sp-2);
    /* Fade out on right edge */
    -webkit-mask-image: linear-gradient(to right, black 0%, black 88%, transparent 100%);
    mask-image: linear-gradient(to right, black 0%, black 88%, transparent 100%);
    scroll-behavior: smooth;
    /* Native momentum scrolling */
    -webkit-overflow-scrolling: touch;
    scrollbar-width: none;
  }
  .shelf-track::-webkit-scrollbar { display: none; }

  /* Placeholder card (Index ecosystems stub) — match real card dims. */
  .placeholder-card {
    width: 280px;
    height: 220px;
    flex-shrink: 0;
    background: var(--surface-1);
    border: 1px dashed var(--border-strong);
    border-radius: var(--r-lg);
    padding: var(--sp-4);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--sp-3);
    text-align: center;
  }

  .placeholder-icon {
    opacity: 0.5;
  }

  .placeholder-label {
    font-size: var(--text-heading);
    font-weight: var(--weight-semi);
    color: var(--fg-muted);
  }

  .placeholder-sub {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: var(--fg-subtle);
    line-height: 1.4;
  }

  .coming-soon {
    display: inline-block;
    padding: 3px var(--sp-3);
    border-radius: var(--r-pill);
    background: var(--surface-3);
    border: 1px solid var(--border-strong);
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--fg-subtle);
  }
</style>

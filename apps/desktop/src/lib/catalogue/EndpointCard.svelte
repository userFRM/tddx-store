<script lang="ts">
  /**
   * Generic endpoint card. One per registry-discovered thetadatadx
   * endpoint. Title = endpoint name, body = description, footer =
   * required-param hint + a "Run once" button that opens the dynamic
   * runner.
   */
  import { Plus, Play, ChevronRight } from "lucide-svelte";
  import type { EndpointInfo } from "$lib/api";
  import { app } from "$lib/stores/app.svelte";

  let { endpoint }: { endpoint: EndpointInfo } = $props();

  const required = $derived(
    endpoint.params.filter((p) => p.required).map((p) => p.name).join(", ") || "—",
  );
  const cadence = $derived(endpoint.subcategory.replace("_", " "));

  function open() {
    app.endpointRunner = { endpoint, args: {}, format: "parquet", busy: false, msg: "" };
    app.endpointRunnerOpen = true;
  }
</script>

<article class="endpoint-card" onclick={open} role="button" tabindex="0"
         onkeydown={(e) => e.key === "Enter" && open()}>
  <div class="head">
    <span class="cat-pill">{endpoint.category}</span>
    <span class="sub-pill">{cadence}</span>
  </div>

  <div class="body">
    <h3 class="ep-name text-mono">{endpoint.name}</h3>
    <p class="ep-desc">{endpoint.description.split(".")[0]}.</p>
  </div>

  <div class="foot">
    <div class="required text-caption">
      <span class="req-key">Required:</span>
      <span class="req-vals text-mono">{required}</span>
    </div>
    <button class="run-btn" onclick={(e) => { e.stopPropagation(); open(); }}>
      <Play size={11} fill="currentColor" />
      Run
    </button>
  </div>
</article>

<style>
  .endpoint-card {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    padding: var(--sp-3) var(--sp-4);
    display: grid;
    grid-template-rows: auto 1fr auto;
    gap: var(--sp-3);
    height: 168px;
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-standard),
                border-color var(--dur-fast) var(--ease-standard),
                transform var(--dur-fast) var(--ease-standard);
    outline: none;
  }
  .endpoint-card:hover {
    background: var(--surface-2);
    border-color: var(--border-strong);
    transform: translateY(-1px);
  }
  .endpoint-card:focus-visible {
    border-color: var(--accent);
    box-shadow: var(--shadow-glow-accent);
  }

  .head {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }
  .cat-pill, .sub-pill {
    display: inline-block;
    padding: 1px 7px;
    border-radius: var(--r-pill);
    font-size: 10px;
    font-weight: var(--weight-semi);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .cat-pill { background: var(--accent-tint); color: var(--accent-hi); }
  .sub-pill { background: var(--surface-3); color: var(--fg-muted); }

  .body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    overflow: hidden;
  }
  .ep-name {
    font-size: var(--text-body);
    font-weight: var(--weight-semi);
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ep-desc {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    line-height: 1.4;
    margin: 0;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--sp-2);
    border-top: 1px solid var(--border);
    padding-top: var(--sp-2);
  }
  .required {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
  }
  .req-key { color: var(--fg-subtle); flex-shrink: 0; }
  .req-vals {
    color: var(--fg-muted);
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .run-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px var(--sp-2);
    background: var(--surface-3);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    color: var(--fg);
    font-size: 11px;
    font-weight: var(--weight-medium);
    cursor: pointer;
    flex-shrink: 0;
  }
  .run-btn:hover {
    background: var(--accent-tint);
    color: var(--accent-hi);
    border-color: rgba(124,140,255,0.3);
  }
</style>

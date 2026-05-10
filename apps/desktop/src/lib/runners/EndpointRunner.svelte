<script lang="ts">
  /**
   * Generic one-shot dispatcher modal: pick any registered endpoint,
   * fill its registry-declared params, dispatch via `endpoint_invoke`,
   * write result to a chosen file. Powers the "Run" button on every
   * EndpointCard.
   */
  import { X, Play, Loader2, Check, ChevronRight } from "lucide-svelte";
  import { api } from "$lib/api";
  import { app, log } from "$lib/stores/app.svelte";

  function close() { app.endpointRunnerOpen = false; }

  const r = $derived(app.endpointRunner);

  function fieldChange(name: string, value: string) {
    if (!app.endpointRunner) return;
    app.endpointRunner.args = { ...app.endpointRunner.args, [name]: value };
  }

  function defaultOutputPath(): string {
    if (!r) return "";
    const stem = `${r.endpoint.name}__${Date.now()}`;
    const ext = r.format;
    return `${app.settings.output_dir}/_oneshot/${stem}.${ext}`;
  }

  async function run() {
    if (!app.endpointRunner) return;
    const ep = app.endpointRunner.endpoint;
    // Required-param validation client-side.
    for (const p of ep.params) {
      if (p.required && !app.endpointRunner.args[p.name]) {
        app.endpointRunner.msg = `Missing required: ${p.name}`;
        return;
      }
    }
    app.endpointRunner.busy = true;
    app.endpointRunner.msg = "Dispatching…";
    const path = defaultOutputPath();
    try {
      const rows = await api.endpointInvoke({
        endpoint: ep.name,
        args: app.endpointRunner.args,
        format: app.endpointRunner.format,
        output_path: path,
      });
      app.endpointRunner.busy = false;
      app.endpointRunner.msg =
        rows === 0 ? "No data" : `Wrote ${rows.toLocaleString()} row${rows === 1 ? "" : "s"} → ${path}`;
      log("info", `Ran ${ep.name}`, { rows, path });
    } catch (e: unknown) {
      app.endpointRunner.busy = false;
      const msg = e instanceof Error ? e.message : String(e);
      app.endpointRunner.msg = msg;
      log("error", `${ep.name} failed: ${msg}`);
    }
  }

  function paramHint(t: string): string {
    const lower = t.toLowerCase();
    if (lower.includes("date")) return "YYYYMMDD";
    if (lower.includes("symbol")) return "QQQ";
    if (lower.includes("strike")) return "* or e.g. 500000";
    if (lower.includes("right"))  return "C, P, or both";
    if (lower.includes("expiration")) return "YYYYMMDD or *";
    if (lower.includes("interval")) return "0 (tick), 1s, 60s";
    if (lower.includes("int") || lower.includes("number")) return "integer";
    if (lower.includes("bool")) return "true / false";
    return "";
  }
</script>

{#if app.endpointRunnerOpen && r}
  <div class="backdrop" onclick={close} role="presentation">
    <div class="card" onclick={(e) => e.stopPropagation()}
         role="dialog" aria-modal="true" tabindex="-1"
         onkeydown={(e) => e.key === "Escape" && close()}>
      <header class="head">
        <div class="title-block">
          <span class="text-caption">Run endpoint</span>
          <h2 class="title text-mono">{r.endpoint.name}</h2>
          <p class="sub fg-muted">{r.endpoint.description.split(".")[0]}.</p>
          <div class="meta tabnum">
            <span class="meta-pill">{r.endpoint.category}</span>
            <span class="meta-pill">{r.endpoint.subcategory}</span>
            <span class="meta-pill">→ {r.endpoint.returns}</span>
          </div>
        </div>
        <button class="btn-icon" onclick={close} aria-label="Close">
          <X size={14} />
        </button>
      </header>

      <div class="form">
        {#each r.endpoint.params as p (p.name)}
          <label class="field">
            <span class="text-caption">
              {p.name}
              {#if p.required}<span class="req">*</span>{/if}
              <span class="ptype">· {p.param_type}</span>
            </span>
            <input
              class="field-input text-mono"
              placeholder={paramHint(p.param_type)}
              value={r.args[p.name] ?? ""}
              oninput={(e) => fieldChange(p.name, (e.target as HTMLInputElement).value)}
            />
            {#if p.description}
              <span class="phint fg-muted">{p.description}</span>
            {/if}
          </label>
        {/each}

        <label class="field">
          <span class="text-caption">Output format</span>
          <select class="field-input"
                  value={r.format}
                  oninput={(e) => app.endpointRunner && (app.endpointRunner.format = (e.target as HTMLSelectElement).value as typeof r.format)}>
            <option value="parquet">Parquet (zstd)</option>
            <option value="csv">CSV</option>
            <option value="jsonl">JSON Lines</option>
            <option value="json">JSON array</option>
          </select>
        </label>
      </div>

      <footer class="foot">
        <span class="msg" class:error={r.msg.toLowerCase().startsWith("missing") || r.msg.toLowerCase().includes("failed")}>
          {r.msg}
        </span>
        <div class="actions">
          <button class="btn btn-ghost" onclick={close}>Close</button>
          <button class="btn btn-primary" onclick={run} disabled={r.busy}>
            {#if r.busy}
              <Loader2 class="spin" size={14} />Dispatching…
            {:else}
              <Play size={14} fill="currentColor" />Run
            {/if}
          </button>
        </div>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed; inset: 0;
    background: rgba(8,11,18,0.55);
    backdrop-filter: blur(4px);
    display: flex; align-items: center; justify-content: center;
    z-index: 90;
  }
  .card {
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-modal);
    width: 620px;
    max-width: calc(100vw - var(--sp-8));
    max-height: calc(100vh - var(--sp-12));
    display: flex; flex-direction: column;
  }
  .head {
    display: flex; justify-content: space-between; gap: var(--sp-3);
    padding: var(--sp-5);
    border-bottom: 1px solid var(--border);
  }
  .title-block { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .title { font-size: 18px; font-weight: var(--weight-semi); margin: 0; color: var(--fg); }
  .sub { font-size: var(--text-body-sm); margin: 0; }
  .meta { display: flex; gap: 6px; margin-top: 6px; }
  .meta-pill {
    font-size: 10px;
    background: var(--surface-3);
    border: 1px solid var(--border);
    color: var(--fg-muted);
    padding: 1px 7px;
    border-radius: var(--r-pill);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-weight: var(--weight-medium);
  }

  .form {
    padding: var(--sp-4) var(--sp-5);
    display: flex; flex-direction: column; gap: var(--sp-3);
    overflow-y: auto;
    flex: 1;
  }
  .field { display: flex; flex-direction: column; gap: 4px; }
  .field .req { color: var(--bad); margin-left: 2px; }
  .ptype { color: var(--fg-subtle); font-family: var(--font-mono); margin-left: 6px; }
  .phint { font-size: 11px; }

  .foot {
    display: flex; justify-content: space-between; align-items: center; gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-5);
    border-top: 1px solid var(--border);
    background: var(--surface-1);
    border-radius: 0 0 var(--r-lg) var(--r-lg);
  }
  .msg { color: var(--fg-muted); font-size: var(--text-body-sm); }
  .msg.error { color: var(--bad); }
  .actions { display: flex; gap: var(--sp-2); }
  :global(.spin) { animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>

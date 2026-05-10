<script lang="ts">
  /**
   * One-shot flatfile downloader. Reads `app.endpointRunner._flatfile`
   * spec set by FlatfilesShelf, asks for a date + output path, hits the
   * flatfile_download Tauri command. Files come back as a CSV / JSONL
   * payload at the given output path (one trading day per request).
   */
  import { X, Loader2, FileArchive, Play } from "lucide-svelte";
  import { app, log } from "$lib/stores/app.svelte";
  import { api } from "$lib/api";

  type FF = {
    title: string;
    sec: "STOCK" | "OPTION";
    req: string;
    desc: string;
  };

  function close() {
    app.flatfileRunnerOpen = false;
    app.endpointRunner = null;
  }

  const ff = $derived(
    (app.endpointRunner as unknown as { _flatfile?: FF } | null)?._flatfile ?? null,
  );

  let date = $state("");
  let format = $state<"CSV" | "JSONL">("CSV");
  let outputPath = $state("");
  let busy = $state(false);
  let msg = $state("");

  $effect(() => {
    if (!app.flatfileRunnerOpen || !ff) return;
    // Suggest a sensible default output path under settings.output_dir.
    if (!outputPath && date && app.settings.output_dir) {
      outputPath = `${app.settings.output_dir}/_flatfiles/${ff.sec.toLowerCase()}_${ff.req.toLowerCase()}_${date}.${format.toLowerCase()}`;
    }
  });

  $effect(() => {
    if (!ff || !date || !app.settings.output_dir) return;
    outputPath = `${app.settings.output_dir}/_flatfiles/${ff.sec.toLowerCase()}_${ff.req.toLowerCase()}_${date}.${format.toLowerCase()}`;
  });

  async function run() {
    if (!ff) return;
    if (!date) { msg = "Date required (YYYYMMDD)."; return; }
    busy = true;
    msg = "Downloading…";
    try {
      const path = await api.flatfileDownload({
        sec_type: ff.sec,
        req_type: ff.req as "TRADE" | "QUOTE" | "TRADE_QUOTE" | "OPEN_INTEREST" | "OHLC" | "EOD",
        date,
        output_path: outputPath,
        format,
      });
      busy = false;
      msg = `Wrote ${path}`;
      log("info", `Flatfile downloaded`, { sec: ff.sec, req: ff.req, date, path });
    } catch (e: unknown) {
      busy = false;
      const m = e instanceof Error ? e.message : String(e);
      msg = m;
      log("error", `Flatfile failed: ${m}`);
    }
  }
</script>

{#if app.flatfileRunnerOpen && ff}
  <div class="backdrop" onclick={close} role="presentation">
    <div class="card" onclick={(e) => e.stopPropagation()}
         role="dialog" aria-modal="true" tabindex="-1"
         onkeydown={(e) => e.key === "Escape" && close()}>
      <header class="head">
        <div>
          <span class="text-caption">Flatfile · bulk-day pull</span>
          <h2 class="title"><FileArchive size={18} /> {ff.title}</h2>
          <p class="sub fg-muted">{ff.desc}</p>
        </div>
        <button class="btn-icon" onclick={close} aria-label="Close"><X size={14} /></button>
      </header>

      <div class="form">
        <label class="field">
          <span class="text-caption">Trading day</span>
          <input class="field-input text-mono" bind:value={date} placeholder="YYYYMMDD" />
        </label>
        <div class="row">
          <label class="field">
            <span class="text-caption">Format</span>
            <select class="field-input" bind:value={format}>
              <option value="CSV">CSV</option>
              <option value="JSONL">JSON Lines</option>
            </select>
          </label>
          <label class="field" />
        </div>
        <label class="field">
          <span class="text-caption">Output path</span>
          <input class="field-input text-mono" bind:value={outputPath} />
        </label>
      </div>

      <footer class="foot">
        <span class="msg" class:error={msg.toLowerCase().includes("required") || msg.toLowerCase().includes("failed")}>
          {msg}
        </span>
        <button class="btn btn-primary" onclick={run} disabled={busy || !date}>
          {#if busy}<Loader2 class="spin" size={14} />Downloading…
          {:else}<Play size={14} fill="currentColor" />Download
          {/if}
        </button>
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
    width: 600px;
    max-width: calc(100vw - var(--sp-8));
  }
  .head {
    display: flex; justify-content: space-between; gap: var(--sp-3);
    padding: var(--sp-5);
    border-bottom: 1px solid var(--border);
  }
  .title {
    display: inline-flex; align-items: center; gap: 10px;
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    margin: 0;
  }
  .sub { font-size: var(--text-body-sm); margin: 4px 0 0; }
  .form {
    padding: var(--sp-4) var(--sp-5);
    display: flex; flex-direction: column; gap: var(--sp-3);
  }
  .row { display: grid; grid-template-columns: 1fr 1fr; gap: var(--sp-3); }
  .field { display: flex; flex-direction: column; gap: 4px; }
  .foot {
    display: flex; justify-content: space-between; align-items: center; gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-5);
    border-top: 1px solid var(--border);
    background: var(--surface-1);
    border-radius: 0 0 var(--r-lg) var(--r-lg);
  }
  .msg { color: var(--fg-muted); font-size: var(--text-body-sm); }
  .msg.error { color: var(--bad); }
  :global(.spin) { animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>

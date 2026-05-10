<script lang="ts">
  /**
   * Read-only telemetry: SDK versions, pool size, in-flight workers,
   * task counts, on-disk totals, uptime. Polls `health` every 3 s
   * because nothing in this surface needs sub-second responsiveness
   * — the queue panel already runs at 1.5 s.
   */
  import { onMount, onDestroy } from "svelte";
  import {
    Activity,
    Cpu,
    Database,
    Download,
    Clock,
    Layers,
    HeartPulse,
    Loader2,
    AlertCircle,
  } from "lucide-svelte";
  import { api, fmtBytes, fmtNum, type HealthSnapshot, TAURI_AVAILABLE } from "$lib/api";
  import { app } from "$lib/stores/app.svelte";

  let snap = $state<HealthSnapshot | null>(null);
  let err = $state<string | null>(null);
  let timer: ReturnType<typeof setInterval> | null = null;

  async function refresh() {
    if (!TAURI_AVAILABLE) return;
    try {
      snap = await api.health();
      err = null;
    } catch (e: unknown) {
      err = e instanceof Error ? e.message : String(e);
    }
  }
  onMount(() => {
    refresh();
    timer = setInterval(refresh, 3000);
  });
  onDestroy(() => timer && clearInterval(timer));

  function fmtUptime(s: number): string {
    if (s < 60) return `${s}s`;
    if (s < 3600) return `${Math.floor(s / 60)}m`;
    if (s < 86400) return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
    return `${Math.floor(s / 86400)}d ${Math.floor((s % 86400) / 3600)}h`;
  }
</script>

<div class="health-view">
  <header>
    <span class="text-caption">Diagnostics</span>
    <h1 class="title">Health</h1>
    <p class="sub fg-muted">
      Pool size, in-flight workers, queue counts, on-disk footprint and
      uptime. Refreshes every 3 seconds while open.
    </p>
  </header>

  {#if err}
    <div class="error-card">
      <AlertCircle size={14} />
      <span>{err}</span>
    </div>
  {:else if !snap}
    <div class="loading">
      <Loader2 size={14} class="spin" />
      <span class="fg-muted">Polling health…</span>
    </div>
  {:else}
    <div class="grid">
      <div class="card">
        <div class="card-head"><Activity size={14} /><span class="text-caption">Connection</span></div>
        <div class="metric"><span class="v" class:ok={app.connState === "connected"} class:warn={app.connState !== "connected"}>{app.connState}</span></div>
        <div class="hint fg-muted">{app.connMsg || "—"}</div>
      </div>

      <div class="card">
        <div class="card-head"><Cpu size={14} /><span class="text-caption">Worker pool</span></div>
        <div class="metric tabnum">
          <span class="v">{snap.workers_in_flight}</span>
          <span class="d">/ {snap.pool_size}</span>
        </div>
        <div class="hint fg-muted">
          {snap.pool_active ? "active" : "idle"} ·
          stock {snap.pool_per_class.stock} · option {snap.pool_per_class.option}
          {#if snap.pool_per_class.index > 0} · index {snap.pool_per_class.index}{/if}
          {#if snap.pool_per_class.rate  > 0} · rate {snap.pool_per_class.rate}{/if}
        </div>
      </div>

      <div class="card">
        <div class="card-head"><HeartPulse size={14} /><span class="text-caption">Tasks</span></div>
        <div class="counts tabnum">
          {#each Object.entries(snap.task_counts) as [k, v]}
            <div class="count-row">
              <span class="k">{k}</span>
              <span class="v">{fmtNum(v)}</span>
            </div>
          {/each}
        </div>
      </div>

      <div class="card">
        <div class="card-head"><Database size={14} /><span class="text-caption">On disk</span></div>
        <div class="metric tabnum">
          <span class="v">{fmtNum(snap.total_files_on_disk)}</span>
          <span class="d">files</span>
        </div>
        <div class="hint fg-muted tabnum">{fmtBytes(snap.total_bytes_on_disk)}</div>
      </div>

      <div class="card">
        <div class="card-head"><Clock size={14} /><span class="text-caption">Uptime</span></div>
        <div class="metric tabnum">
          <span class="v">{fmtUptime(snap.uptime_secs)}</span>
        </div>
      </div>

      <div class="card">
        <div class="card-head"><Layers size={14} /><span class="text-caption">Build versions</span></div>
        <div class="versions">
          <div class="ver-row">
            <span class="ver-name">tdds-desktop</span>
            <code>{snap.desktop_version}</code>
          </div>
          <div class="ver-row">
            <span class="ver-name">thetadatadx</span>
            <code>{snap.thetadatadx_version}</code>
          </div>
          <div class="ver-row">
            <span class="ver-name">tdbe</span>
            <code>{snap.tdbe_version}</code>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .health-view {
    padding: var(--sp-8);
    display: flex; flex-direction: column; gap: var(--sp-5);
    overflow-y: auto; height: 100%;
    max-width: 1100px;
  }
  header { display: flex; flex-direction: column; gap: 4px; }
  .title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    margin: 0;
  }
  .sub { font-size: var(--text-body); margin: 0; max-width: 640px; }

  .error-card {
    display: inline-flex; align-items: center; gap: 6px;
    padding: var(--sp-3) var(--sp-4);
    border: 1px solid rgba(255,126,126,0.3);
    background: rgba(255,126,126,0.06);
    color: var(--bad);
    border-radius: var(--r-sm);
  }
  .loading { display: inline-flex; align-items: center; gap: 6px; padding: var(--sp-4); }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: var(--sp-3);
  }
  .card {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    padding: var(--sp-4);
    display: flex; flex-direction: column; gap: var(--sp-2);
  }
  .card-head {
    display: inline-flex; align-items: center; gap: 6px;
    color: var(--fg-muted);
  }
  .metric { font-family: var(--font-mono); display: inline-flex; align-items: baseline; gap: 4px; }
  .metric .v { font-size: 28px; font-weight: var(--weight-semi); color: var(--fg); }
  .metric .v.ok { color: var(--good); font-size: 22px; text-transform: capitalize; }
  .metric .v.warn { color: var(--warn); font-size: 22px; text-transform: capitalize; }
  .metric .d { color: var(--fg-muted); font-size: 14px; }

  .hint { font-size: var(--text-body-sm); }

  .counts { display: flex; flex-direction: column; gap: 4px; }
  .count-row {
    display: flex; justify-content: space-between;
    font-size: var(--text-body-sm);
    color: var(--fg);
  }
  .count-row .k { color: var(--fg-muted); text-transform: capitalize; }

  .versions { display: flex; flex-direction: column; gap: 4px; }
  .ver-row {
    display: flex; justify-content: space-between;
    font-size: var(--text-body-sm);
  }
  .ver-row .ver-name { color: var(--fg-muted); }
  .ver-row code {
    font-family: var(--font-mono);
    background: var(--surface-3);
    padding: 1px 6px;
    border-radius: var(--r-sm);
    color: var(--fg);
  }
</style>

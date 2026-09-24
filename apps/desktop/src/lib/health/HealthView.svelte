<script lang="ts">
  /**
   * Read-only telemetry: SDK versions, pool size, in-flight workers,
   * task counts, on-disk totals, uptime. Re-reads `health` whenever the
   * queue or the library changes, which is when any of it can move;
   * uptime counts on locally between reads.
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
  let clock: ReturnType<typeof setInterval> | null = null;
  /** When `snap` was read, so uptime can advance without asking again. */
  let readAt = $state(0);
  let now = $state(Date.now());
  const uptime = $derived(snap ? snap.uptime_secs + Math.max(0, Math.floor((now - readAt) / 1000)) : 0);

  let reading = false;
  let readAgain = false;
  async function refresh() {
    if (!TAURI_AVAILABLE) return;
    // One read at a time; changes during it earn one more read after.
    if (reading) {
      readAgain = true;
      return;
    }
    reading = true;
    try {
      snap = await api.health();
      readAt = Date.now();
      now = readAt;
      err = null;
    } catch (e: unknown) {
      err = e instanceof Error ? e.message : String(e);
    } finally {
      reading = false;
    }
    if (readAgain) {
      readAgain = false;
      void refresh();
    }
  }
  // Pool, counts and footprint change exactly when the queue snapshot or
  // the library does; follow those instead of a timer.
  $effect(() => {
    void app.queueSnap;
    void app.libraryRev;
    void app.runningTaskIds.length;
    void refresh();
  });
  onMount(() => {
    // Display only: no call to the backend.
    clock = setInterval(() => (now = Date.now()), 1000);
  });
  onDestroy(() => clock && clearInterval(clock));

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
      uptime. Updates as the queue and the library change.
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
          {snap.pool_active ? "active" : "idle"} · account-wide budget
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
          {:else}
            <!-- The backend sends counts only for statuses that have
                 rows, so a queue that has never run sends none and this
                 card rendered as an empty box with a title. -->
            <p class="hint fg-muted">Queue is empty.</p>
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
          <span class="v">{fmtUptime(uptime)}</span>
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
    border: 1px solid var(--bad-tint);
    background: var(--bad-tint);
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

<script lang="ts">
  /**
   * Right-side download pane. Modeled after Transmission / qBittorrent:
   * shows running tasks with live progress bars, throughput, ETA, and
   * quick row actions. Auto-hides when there is no activity (no running,
   * no pending, no failed); collapsible to a thin summary strip via the
   * header chevron when the user wants the screen real estate back.
   */
  import { onDestroy } from "svelte";
  import {
    Play,
    Pause,
    RotateCcw,
    Download,
    X,
    CheckCircle2,
    XCircle,
    Loader2,
    GripVertical,
    ChevronRight,
    ChevronLeft,
  } from "lucide-svelte";
  import { app } from "$lib/stores/app.svelte";
  import { api, fmtBytes, fmtNum, type TaskView } from "$lib/api";

  // ── Throughput estimator (rolling, from successive snapshots) ─────
  let prevBytes = $state(0);
  let prevDone = $state(0);
  let prevTs = $state(0);
  let bytesPerSec = $state(0);
  let tasksPerSec = $state(0);

  $effect(() => {
    const s = app.queueSnap;
    if (!s) return;
    const now = performance.now();
    const dt = (now - prevTs) / 1000;
    if (prevTs > 0 && dt >= 0.5) {
      const dBytes = s.bytes_on_disk - prevBytes;
      const dDone =
        (s.counts.find(([k]) => k === "done")?.[1] ?? 0) - prevDone;
      // EMA smoothing so the number doesn't jitter
      const alpha = 0.4;
      bytesPerSec = bytesPerSec * (1 - alpha) + Math.max(0, dBytes / dt) * alpha;
      tasksPerSec = tasksPerSec * (1 - alpha) + Math.max(0, dDone / dt) * alpha;
    }
    prevBytes = s.bytes_on_disk;
    prevDone = s.counts.find(([k]) => k === "done")?.[1] ?? 0;
    prevTs = now;
  });

  const running = $derived<TaskView[]>(
    (app.queueSnap?.recent ?? []).filter(t => t.status === "running"),
  );
  const recentDone = $derived<TaskView[]>(
    (app.queueSnap?.recent ?? [])
      .filter(t => t.status === "done" || t.status === "empty")
      .slice(0, 6),
  );
  const recentFailed = $derived<TaskView[]>(
    (app.queueSnap?.recent ?? []).filter(t => t.status === "failed").slice(0, 4),
  );

  const counts = $derived(app.queueSnap?.counts ?? []);
  const pending = $derived(counts.find(([k]) => k === "pending")?.[1] ?? 0);
  const doneN   = $derived(counts.find(([k]) => k === "done")?.[1]    ?? 0);
  const failed  = $derived(counts.find(([k]) => k === "failed")?.[1]  ?? 0);

  const etaSec = $derived(() => {
    const inflight = pending + running.length;
    if (inflight === 0 || tasksPerSec <= 0) return null;
    return inflight / tasksPerSec;
  });

  function fmtETA(s: number | null): string {
    if (s === null || !isFinite(s)) return "—";
    if (s < 60) return `${Math.round(s)}s`;
    if (s < 3600) return `${Math.floor(s / 60)}m ${Math.round(s % 60)}s`;
    const h = Math.floor(s / 3600);
    const m = Math.round((s % 3600) / 60);
    return `${h}h ${m}m`;
  }

  function fmtRate(bps: number): string {
    if (bps < 1024) return `${bps.toFixed(0)} B/s`;
    if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(1)} KB/s`;
    if (bps < 1024 ** 3) return `${(bps / 1024 / 1024).toFixed(1)} MB/s`;
    return `${(bps / 1024 ** 3).toFixed(2)} GB/s`;
  }

  // Indeterminate fake progress: server doesn't report row-level progress
  // mid-call, but the UI feels dead without motion. We tween a fraction
  // toward 95% per running task; on completion the task disappears.
  let runningFracs = $state<Record<string, number>>({});
  const TICK_MS = 200;

  let tickTimer: ReturnType<typeof setInterval> | null = null;
  function startTick() {
    if (tickTimer !== null) return;
    tickTimer = setInterval(() => {
      const next: Record<string, number> = {};
      for (const r of running) {
        const cur = runningFracs[r.id] ?? 0.04;
        // asymptotic creep toward 0.95
        next[r.id] = cur + (0.95 - cur) * 0.06;
      }
      runningFracs = next;
    }, TICK_MS);
  }
  function stopTick() {
    if (tickTimer !== null) {
      clearInterval(tickTimer);
      tickTimer = null;
    }
  }

  $effect(() => {
    if (running.length > 0) startTick();
    else stopTick();
  });

  onDestroy(stopTick);

  async function startWorkers() {
    try { await api.runQueue(); } catch {}
  }
  async function retryFailed() {
    try { await api.requeueFailed(); } catch {}
  }

  // ── Visibility + collapse state ─────────────────────────────────
  // Hide entirely when nothing is happening; collapse to a thin strip
  // (header only) when the user wants the screen real estate back.
  // Both states are session-local — no kv persistence since they
  // change with traffic, not user preference.
  const hasActivity = $derived(
    running.length > 0 ||
      pending > 0 ||
      failed > 0 ||
      recentDone.length > 0,
  );
  let collapsed = $state(false);
  function toggleCollapsed() { collapsed = !collapsed; }
</script>

{#if hasActivity}
<aside class="downloads-pane" class:collapsed aria-label="Active downloads">
  <header class="pane-header">
    <div class="pane-title-row">
      <button
        class="btn-icon collapse-btn"
        onclick={toggleCollapsed}
        title={collapsed ? "Expand downloads pane" : "Collapse downloads pane"}
        aria-expanded={!collapsed}
      >
        {#if collapsed}
          <ChevronLeft size={14} />
        {:else}
          <ChevronRight size={14} />
        {/if}
      </button>
      <span class="text-caption">Downloads</span>
      <div class="pane-actions">
        {#if pending > 0 && running.length === 0}
          <button class="btn-icon" onclick={startWorkers} title="Start workers">
            <Play size={14} />
          </button>
        {/if}
        {#if failed > 0}
          <button class="btn-icon" onclick={retryFailed} title="Retry failed">
            <RotateCcw size={14} />
          </button>
        {/if}
      </div>
    </div>

    <div class="pane-summary tabnum">
      <div class="summary-row">
        <span class="summary-label">Active</span>
        <span class="summary-value running-count">
          {fmtNum(running.length)}
          {#if running.length > 0}
            <Loader2 class="spin" size={12} />
          {/if}
        </span>
      </div>
      <div class="summary-row">
        <span class="summary-label">Pending</span>
        <span class="summary-value">{fmtNum(pending)}</span>
      </div>
      <div class="summary-row">
        <span class="summary-label">Done</span>
        <span class="summary-value good">{fmtNum(doneN)}</span>
      </div>
      {#if failed > 0}
        <div class="summary-row">
          <span class="summary-label">Failed</span>
          <span class="summary-value bad">{fmtNum(failed)}</span>
        </div>
      {/if}
      <div class="divider"></div>
      <div class="summary-row">
        <span class="summary-label">Throughput</span>
        <span class="summary-value">{fmtRate(bytesPerSec)}</span>
      </div>
      <div class="summary-row">
        <span class="summary-label">ETA</span>
        <span class="summary-value">{fmtETA(etaSec())}</span>
      </div>
    </div>
  </header>

  {#if !collapsed}
  <div class="pane-scroll">
    <!-- ── Active running ────────────────────────────────────── -->
    {#if running.length > 0}
      <section class="section">
        <div class="section-header">
          <span class="text-caption">Active · {running.length}</span>
        </div>
        <ul class="task-list">
          {#each running as t (t.id)}
            <li class="task running">
              <div class="task-head">
                <div class="task-title">
                  <span class="task-symbol">{t.symbol}</span>
                  <span class="task-kind text-mono">{t.kind}</span>
                </div>
                <span class="task-date text-mono">{t.date}</span>
              </div>
              <div class="progress-track">
                <div
                  class="progress-fill running"
                  style:width="{(runningFracs[t.id] ?? 0.04) * 100}%"
                ></div>
              </div>
              <div class="task-meta tabnum">
                <span class="meta-pct">{Math.round((runningFracs[t.id] ?? 0.04) * 100)}%</span>
                <span class="sep">·</span>
                <span>streaming…</span>
                <span class="sep">·</span>
                <span class="fg-muted">attempt {t.attempts}</span>
              </div>
            </li>
          {/each}
        </ul>
      </section>
    {:else if pending > 0}
      <section class="section">
        <div class="empty-active">
          <Pause size={20} />
          <p class="text-body-sm fg-muted">{fmtNum(pending)} queued — workers idle.</p>
          <button class="btn btn-primary" onclick={startWorkers}>
            <Play size={14} fill="currentColor" />
            Start
          </button>
        </div>
      </section>
    {:else if app.connState !== "connected"}
      <section class="section">
        <div class="empty-active">
          <Download size={20} />
          <p class="text-body-sm fg-muted">Connect in Settings to start downloading.</p>
        </div>
      </section>
    {:else}
      <section class="section">
        <div class="empty-active">
          <Download size={20} />
          <p class="text-body-sm fg-muted">Queue is empty.<br />Browse a dataset to add downloads.</p>
        </div>
      </section>
    {/if}

    <!-- ── Recently failed ────────────────────────────────────── -->
    {#if recentFailed.length > 0}
      <section class="section">
        <div class="section-header">
          <span class="text-caption">Failed</span>
          <button class="btn-icon" onclick={retryFailed} title="Retry all failed">
            <RotateCcw size={12} />
          </button>
        </div>
        <ul class="task-list">
          {#each recentFailed as t (t.id)}
            <li class="task failed">
              <div class="task-head">
                <div class="task-title">
                  <span class="task-icon"><XCircle size={12} /></span>
                  <span class="task-symbol">{t.symbol}</span>
                  <span class="task-kind text-mono">{t.kind}</span>
                </div>
                <span class="task-date text-mono">{t.date}</span>
              </div>
              {#if t.error}
                <div class="task-error text-body-sm" title={t.error}>
                  {t.error.length > 60 ? t.error.slice(0, 60) + "…" : t.error}
                </div>
              {/if}
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <!-- ── Recently done ──────────────────────────────────────── -->
    {#if recentDone.length > 0}
      <section class="section">
        <div class="section-header">
          <span class="text-caption">Recently complete</span>
        </div>
        <ul class="task-list">
          {#each recentDone as t (t.id)}
            <li class="task done">
              <div class="task-head">
                <div class="task-title">
                  <span class="task-icon"><CheckCircle2 size={12} /></span>
                  <span class="task-symbol">{t.symbol}</span>
                  <span class="task-kind text-mono">{t.kind}</span>
                </div>
                <span class="task-date text-mono">{t.date}</span>
              </div>
              <div class="task-meta tabnum">
                <span>{fmtNum(t.rows)} rows</span>
                <span class="sep">·</span>
                <span>{fmtBytes(t.bytes)}</span>
              </div>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  </div>
  {/if}
</aside>
{/if}

<style>
  .downloads-pane {
    width: 320px;
    flex-shrink: 0;
    background: var(--surface-1);
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    transition: width var(--dur-base) var(--ease-standard);
  }
  .downloads-pane.collapsed { width: 56px; }
  .downloads-pane.collapsed .pane-summary,
  .downloads-pane.collapsed .text-caption { display: none; }
  .downloads-pane.collapsed .pane-actions { display: none; }
  .downloads-pane.collapsed .pane-title-row { justify-content: center; margin-bottom: 0; }
  .downloads-pane.collapsed .pane-header { padding: var(--sp-3) var(--sp-2); }
  @media (prefers-reduced-motion: reduce) {
    .downloads-pane { transition: none; }
  }
  .collapse-btn { margin-right: var(--sp-2); }

  .pane-header {
    padding: var(--sp-4) var(--sp-4) var(--sp-3);
    border-bottom: 1px solid var(--border);
    background: var(--surface-1);
    flex-shrink: 0;
  }

  .pane-title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--sp-3);
  }
  .pane-actions {
    display: flex;
    gap: var(--sp-1);
  }

  .pane-summary {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .summary-row {
    display: flex;
    justify-content: space-between;
    font-size: var(--text-body-sm);
    line-height: 1.4;
  }
  .summary-label {
    color: var(--fg-muted);
  }
  .summary-value {
    font-family: var(--font-mono);
    color: var(--fg);
    font-variant-numeric: tabular-nums;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .summary-value.good { color: var(--good); }
  .summary-value.bad  { color: var(--bad); }
  .running-count :global(.spin) {
    animation: spin 0.8s linear infinite;
    color: var(--accent);
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .pane-scroll {
    flex: 1;
    overflow-y: auto;
    padding: var(--sp-2) 0;
  }

  .section {
    padding: var(--sp-3) var(--sp-4);
  }
  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--sp-2);
  }

  .task-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .task {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    padding: var(--sp-2) var(--sp-3);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    transition: background var(--dur-fast) var(--ease-standard),
                border-color var(--dur-fast) var(--ease-standard);
  }
  .task:hover { background: var(--surface-3); border-color: var(--border-strong); }
  .task.running {
    border-left: 2px solid var(--accent);
    padding-left: calc(var(--sp-3) - 1px);
  }
  .task.failed {
    border-left: 2px solid var(--bad);
    padding-left: calc(var(--sp-3) - 1px);
  }
  .task.done {
    opacity: 0.78;
  }

  .task-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--sp-2);
  }
  .task-title {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
  }
  .task-icon {
    display: inline-flex;
    flex-shrink: 0;
  }
  .task.done .task-icon  { color: var(--good); }
  .task.failed .task-icon { color: var(--bad); }

  .task-symbol {
    font-weight: var(--weight-semi);
    font-size: var(--text-body-sm);
    color: var(--fg);
  }
  .task-kind {
    font-size: var(--text-caption);
    color: var(--fg-muted);
    text-transform: lowercase;
    letter-spacing: 0;
  }
  .task-date {
    font-size: var(--text-caption);
    color: var(--fg-muted);
    flex-shrink: 0;
  }

  .task-meta {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: var(--text-caption);
    color: var(--fg-muted);
    text-transform: none;
    letter-spacing: 0;
    font-weight: var(--weight-normal);
  }
  .meta-pct {
    color: var(--accent-hi);
    font-weight: var(--weight-semi);
  }
  .sep { color: var(--fg-subtle); }

  .task-error {
    color: var(--bad);
    font-family: var(--font-mono);
    font-size: var(--text-caption);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-transform: none;
    letter-spacing: 0;
  }

  .empty-active {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--sp-3);
    padding: var(--sp-8) var(--sp-4);
    color: var(--fg-subtle);
    text-align: center;
  }

  .divider {
    margin: var(--sp-2) 0;
  }
</style>

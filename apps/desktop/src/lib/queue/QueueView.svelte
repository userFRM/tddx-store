<script lang="ts">
  import {
    Play,
    Pause,
    RotateCcw,
    ArrowUp,
    X,
    Copy,
    FolderOpen,
    CheckCircle2,
    XCircle,
    Clock,
    Circle,
    AlertCircle,
  } from "lucide-svelte";
  import { app } from "$lib/stores/app.svelte";
  import { api, fmtBytes, fmtNum } from "$lib/api";

  type StatusFilter = "all" | "pending" | "running" | "done" | "failed" | "empty";

  let activeFilter = $state<StatusFilter>("all");
  let workersRunning = $state(false);
  let actionMsg = $state("");

  const snap = $derived(app.queueSnap);

  const totalCount = $derived(
    snap?.counts.reduce((sum, [, n]) => sum + n, 0) ?? 0
  );

  const runningCount = $derived(snap?.counts.find(([s]) => s === "running")?.[1] ?? 0);
  const pendingCount = $derived(snap?.counts.find(([s]) => s === "pending")?.[1] ?? 0);
  const doneCount    = $derived(snap?.counts.find(([s]) => s === "done")?.[1] ?? 0);
  const failedCount  = $derived(snap?.counts.find(([s]) => s === "failed")?.[1] ?? 0);
  const emptyCount   = $derived(snap?.counts.find(([s]) => s === "empty")?.[1] ?? 0);

  const filteredRows = $derived(() => {
    const rows = snap?.recent ?? [];
    if (activeFilter === "all") return rows;
    return rows.filter(r => r.status === activeFilter);
  });

  async function startWorkers() {
    workersRunning = true;
    try {
      await api.runQueue();
      actionMsg = "Workers started";
      setTimeout(() => (actionMsg = ""), 2000);
    } catch (e: unknown) {
      actionMsg = e instanceof Error ? e.message : String(e);
    }
  }

  async function retryFailed() {
    try {
      const n = await api.requeueFailed();
      actionMsg = `Requeued ${n} task${n === 1 ? "" : "s"}`;
      setTimeout(() => (actionMsg = ""), 2000);
    } catch (e: unknown) {
      actionMsg = e instanceof Error ? e.message : String(e);
    }
  }

  function etaStr(): string {
    if (!snap) return "—";
    const done = snap.counts.find(([s]) => s === "done")?.[1] ?? 0;
    const running = snap.counts.find(([s]) => s === "running")?.[1] ?? 0;
    const pending = snap.counts.find(([s]) => s === "pending")?.[1] ?? 0;
    if (done === 0 || (running + pending) === 0) return "—";
    // Rough ETA: not meaningful without per-task timing, so show pending count
    return `${pending + running} remaining`;
  }

  const STATUS_LABELS: Record<StatusFilter, string> = {
    all:     "All",
    pending: "Pending",
    running: "Running",
    done:    "Done",
    failed:  "Failed",
    empty:   "Empty",
  };
</script>

<div class="queue-view">
  <!-- Header bar -->
  <div class="queue-header">
    <div class="header-left">
      <h1 class="queue-title">Queue</h1>
      {#if snap}
        <span class="queue-meta text-mono">
          <span>{fmtNum(totalCount)} tasks</span>
          <span class="sep">·</span>
          <span>{fmtBytes(snap.bytes_on_disk)} on disk</span>
          <span class="sep">·</span>
          <span>{snap.files_on_disk} files</span>
          {#if etaStr() !== "—"}
            <span class="sep">·</span>
            <span>{etaStr()}</span>
          {/if}
        </span>
      {/if}
    </div>

    <div class="header-actions">
      {#if actionMsg}
        <span class="action-feedback text-body-sm">{actionMsg}</span>
      {/if}
      <button class="btn btn-primary" onclick={startWorkers} disabled={workersRunning && runningCount > 0}>
        <Play size={13} strokeWidth={1.75} />
        Start workers
      </button>
      {#if failedCount > 0}
        <button class="btn btn-secondary" onclick={retryFailed}>
          <RotateCcw size={13} strokeWidth={1.75} />
          Retry failed ({failedCount})
        </button>
      {/if}
    </div>
  </div>

  <!-- Status filter pills -->
  <div class="filter-bar" role="group" aria-label="Status filter">
    {#each (["all", "pending", "running", "done", "failed", "empty"] as StatusFilter[]) as f}
      {@const count = f === "all" ? totalCount
        : f === "pending" ? pendingCount
        : f === "running" ? runningCount
        : f === "done"    ? doneCount
        : f === "failed"  ? failedCount
        : emptyCount}
      <button
        class="filter-pill"
        class:active={activeFilter === f}
        class:filter-running={f === "running"}
        class:filter-done={f === "done"}
        class:filter-failed={f === "failed"}
        class:filter-empty={f === "empty"}
        onclick={() => (activeFilter = f)}
        aria-pressed={activeFilter === f}
      >
        {STATUS_LABELS[f]}
        <span class="filter-count text-mono">{fmtNum(count)}</span>
      </button>
    {/each}
  </div>

  <!-- Task rows -->
  <div class="task-list" role="list">
    {#if app.connState !== "connected" && (snap === null || totalCount === 0)}
      <div class="empty-queue">
        <div class="empty-icon" aria-hidden="true">
          <svg width="40" height="40" viewBox="0 0 40 40" fill="none">
            <rect x="6" y="6" width="28" height="28" rx="8" stroke="var(--border-strong)" stroke-width="1.5" />
            <path d="M14 20h12M20 14v12" stroke="var(--fg-subtle)" stroke-width="1.5" stroke-linecap="round" />
          </svg>
        </div>
        <p class="empty-label">Queue is empty</p>
        <p class="empty-sub text-body-sm fg-muted">
          Browse datasets and add items to get started.
          Connect to ThetaData first from Settings.
        </p>
      </div>
    {:else}
      {#each filteredRows() as task (task.id)}
        <div class="task-row" role="listitem" class:row-running={task.status === "running"}>
          <!-- Status indicator -->
          <div class="task-status" aria-label="Status: {task.status}">
            {#if task.status === "running"}
              <Circle size={14} strokeWidth={1.75} style="color: var(--accent); animation: pulse-dot 1.2s ease-in-out infinite;" />
            {:else if task.status === "done"}
              <CheckCircle2 size={14} strokeWidth={1.75} style="color: var(--good);" />
            {:else if task.status === "failed"}
              <XCircle size={14} strokeWidth={1.75} style="color: var(--bad);" />
            {:else if task.status === "empty"}
              <AlertCircle size={14} strokeWidth={1.75} style="color: var(--warn);" />
            {:else}
              <Clock size={14} strokeWidth={1.75} style="color: var(--fg-subtle);" />
            {/if}
          </div>

          <!-- Main content -->
          <div class="task-main">
            <div class="task-top">
              <code class="task-kind">{task.kind}</code>
              <span class="task-symbol">{task.symbol}</span>
              <span class="task-date text-mono">{task.date}</span>
            </div>

            {#if task.status === "running"}
              <!--
                Indeterminate progress: the SDK doesn't surface row-level
                progress mid-call, so a static % was a lie. Use a CSS
                shimmer that pings 30%-70% so the user reads "in flight,
                still alive" without faking a fixed completion ratio.
              -->
              <div class="task-progress">
                <div class="progress-track">
                  <div class="progress-fill running indeterminate"></div>
                </div>
              </div>
            {/if}

            {#if task.error}
              <div class="task-error text-body-sm">{task.error}</div>
            {/if}
          </div>

          <!-- Stats -->
          <div class="task-stats">
            {#if task.rows != null}
              <span class="stat text-mono">{fmtNum(task.rows)} rows</span>
            {/if}
            {#if task.bytes != null}
              <span class="stat text-mono">{fmtBytes(task.bytes)}</span>
            {/if}
            {#if task.attempts > 1}
              <span class="stat attempts">attempt {task.attempts}</span>
            {/if}
          </div>

          <!-- Row actions (hover) -->
          <div class="task-actions" aria-label="Row actions">
            {#if task.status === "pending"}
              <button class="btn-icon" title="Bump priority" aria-label="Bump priority">
                <ArrowUp size={13} strokeWidth={1.75} />
              </button>
            {/if}
            <button class="btn-icon" title="Duplicate" aria-label="Duplicate task">
              <Copy size={13} strokeWidth={1.75} />
            </button>
            {#if task.status === "done"}
              <button class="btn-icon" title="Open file location" aria-label="Open file location">
                <FolderOpen size={13} strokeWidth={1.75} />
              </button>
            {/if}
            {#if task.status !== "done"}
              <button class="btn-icon danger" title="Cancel" aria-label="Cancel task">
                <X size={13} strokeWidth={1.75} />
              </button>
            {/if}
          </div>
        </div>
      {/each}

      {#if filteredRows().length === 0 && totalCount > 0}
        <div class="empty-filter">
          <p class="text-body-sm fg-muted">No {activeFilter} tasks.</p>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .queue-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  /* Header */
  .queue-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
    padding: var(--sp-6) var(--sp-8) var(--sp-4);
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
  }

  .header-left {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .queue-title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    letter-spacing: -0.015em;
    color: var(--fg);
    line-height: 1.15;
  }

  .queue-meta {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
  }

  .sep { color: var(--fg-subtle); }

  .header-actions {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex-shrink: 0;
  }

  .action-feedback {
    color: var(--good);
    animation: fadeInOut 2s ease-in-out forwards;
  }

  @keyframes fadeInOut {
    0%   { opacity: 0; transform: translateY(4px); }
    15%  { opacity: 1; transform: translateY(0); }
    80%  { opacity: 1; }
    100% { opacity: 0; }
  }

  /* Filter bar */
  .filter-bar {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-8);
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
    overflow-x: auto;
    scrollbar-width: none;
  }
  .filter-bar::-webkit-scrollbar { display: none; }

  .filter-pill {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-1) var(--sp-3);
    border-radius: var(--r-pill);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    color: var(--fg-muted);
    background: transparent;
    border: 1px solid transparent;
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard),
      border-color var(--dur-fast) var(--ease-standard);
    outline: none;
    white-space: nowrap;
  }
  .filter-pill:hover { background: var(--surface-2); color: var(--fg); }
  .filter-pill:focus-visible { box-shadow: var(--shadow-glow-accent); }
  .filter-pill.active {
    background: var(--surface-2);
    color: var(--fg);
    border-color: var(--border-strong);
  }
  .filter-pill.active.filter-running { color: var(--accent-hi); border-color: var(--accent-tint); background: var(--accent-tint); }
  .filter-pill.active.filter-done    { color: var(--good); border-color: rgba(93,212,160,0.2); background: rgba(93,212,160,0.08); }
  .filter-pill.active.filter-failed  { color: var(--bad);  border-color: rgba(255,126,126,0.2); background: rgba(255,126,126,0.08); }
  .filter-pill.active.filter-empty   { color: var(--warn); border-color: rgba(245,197,111,0.2); background: rgba(245,197,111,0.08); }

  .filter-count {
    font-size: var(--text-caption);
    color: var(--fg-subtle);
    min-width: 1.5ch;
    text-align: right;
  }
  .filter-pill.active .filter-count { color: inherit; opacity: 0.7; }

  /* Task list */
  .task-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--sp-2) 0;
  }

  .task-row {
    display: grid;
    grid-template-columns: 20px 1fr auto auto;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-3) var(--sp-8);
    border-bottom: 1px solid var(--border);
    transition: background var(--dur-fast) var(--ease-standard);
    position: relative;
  }
  .task-row:last-child { border-bottom: none; }
  .task-row:hover { background: var(--surface-1); }

  .task-row.row-running {
    background: rgba(124, 140, 255, 0.03);
  }

  .task-status {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .task-main {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    min-width: 0;
  }

  .task-top {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex-wrap: wrap;
  }

  .task-kind {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    color: var(--fg-muted);
    background: var(--surface-2);
    padding: 1px var(--sp-2);
    border-radius: var(--r-sm);
  }

  .task-symbol {
    font-weight: var(--weight-semi);
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    color: var(--fg);
    font-variant-numeric: tabular-nums;
  }

  .task-date {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
  }

  .task-progress {
    margin-top: var(--sp-1);
    max-width: 240px;
  }

  .task-error {
    color: var(--bad);
    font-size: var(--text-caption);
    margin-top: 2px;
  }

  /* Stats */
  .task-stats {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex-shrink: 0;
  }

  .stat {
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .attempts {
    font-family: var(--font-ui);
    color: var(--warn);
    font-size: var(--text-caption);
  }

  /* Row actions */
  .task-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-standard);
    flex-shrink: 0;
  }
  .task-row:hover .task-actions { opacity: 1; }

  .btn-icon.danger:hover { color: var(--bad); }

  /* Empty states */
  .empty-queue {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--sp-4);
    height: 100%;
    min-height: 300px;
    padding: var(--sp-8);
    text-align: center;
  }

  .empty-icon { opacity: 0.5; }

  .empty-label {
    font-size: var(--text-heading);
    font-weight: var(--weight-semi);
    color: var(--fg-muted);
  }

  .empty-sub { max-width: 320px; }

  .empty-filter {
    padding: var(--sp-6) var(--sp-8);
  }
</style>

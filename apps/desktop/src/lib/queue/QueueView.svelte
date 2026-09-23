<script lang="ts">
  /**
   * The download queue: filter, select, act.
   *
   * Selection is the reason this view exists in list form — a queue of
   * a few thousand rows is only workable if you can pick a slice and
   * act on all of it at once. Shift-click extends a range, the header
   * box toggles everything visible, and the action bar appears only
   * when something is selected so the resting state stays quiet.
   *
   * The snapshot carries the most recent rows, while the status counts
   * cover the whole table. Anything that acts on "everything" therefore
   * goes through a backend command that works in SQL, never by looping
   * over what happens to be loaded.
   */
  import {
    Play,
    RotateCcw,
    ArrowUp,
    X,
    Copy,
    FolderOpen,
    Trash2,
    CheckCircle2,
    XCircle,
    Clock,
    Circle,
    AlertCircle,
    Search,
    ArrowRight,
  } from "lucide-svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import TransfersList from "$lib/queue/TransfersList.svelte";
  import { app, log, navigate, refreshQueueSnapshot } from "$lib/stores/app.svelte";
  import { api, fmtBytes, fmtNum, type TaskView } from "$lib/api";

  type StatusFilter = "all" | "pending" | "running" | "paused" | "done" | "failed" | "empty";

  const STATUS_LABELS: Record<StatusFilter, string> = {
    all: "All",
    pending: "Pending",
    running: "Running",
    paused: "Paused",
    done: "Done",
    failed: "Failed",
    empty: "Empty",
  };
  const FILTERS = Object.keys(STATUS_LABELS) as StatusFilter[];

  /** Downloads as asked for, or the tasks they were split into. The
   *  first is what most people want; the second is where to go when
   *  one of them misbehaves. */
  let layout = $state<"transfers" | "tasks">("transfers");
  let activeFilter = $state<StatusFilter>("all");
  let query = $state("");
  let selected = $state<Set<string>>(new Set());
  let lastClicked = $state<string | null>(null);
  let busy = $state(false);
  let actionMsg = $state("");

  const snap = $derived(app.queueSnap);
  const counts = $derived(new Map(snap?.counts ?? []));
  const totalCount = $derived([...counts.values()].reduce((a, b) => a + b, 0));
  const countOf = (f: StatusFilter) =>
    f === "all" ? totalCount : (counts.get(f) ?? 0);
  const finishedCount = $derived(
    countOf("done") + countOf("failed") + countOf("empty"),
  );

  function clearFilters() {
    activeFilter = "all";
    query = "";
  }

  const rows = $derived.by<TaskView[]>(() => {
    const q = query.trim().toLowerCase();
    return (snap?.recent ?? []).filter((r) => {
      if (activeFilter !== "all" && r.status !== activeFilter) return false;
      if (!q) return true;
      return (
        r.symbol.toLowerCase().includes(q) ||
        r.kind.toLowerCase().includes(q) ||
        r.date.includes(q)
      );
    });
  });

  // Selection only ever refers to rows the user can currently see, so a
  // filter change drops anything that scrolled out of scope.
  $effect(() => {
    const visible = new Set(rows.map((r) => r.id));
    if ([...selected].every((id) => visible.has(id))) return;
    selected = new Set([...selected].filter((id) => visible.has(id)));
  });

  const selectedRows = $derived(rows.filter((r) => selected.has(r.id)));
  const allVisibleSelected = $derived(
    rows.length > 0 && rows.every((r) => selected.has(r.id)),
  );
  const someVisibleSelected = $derived(selected.size > 0 && !allVisibleSelected);
  const canCancel = $derived(
    selectedRows.some((r) => r.status === "pending" || r.status === "running"),
  );

  function toggleAllVisible() {
    selected = allVisibleSelected ? new Set() : new Set(rows.map((r) => r.id));
  }

  /** Plain click toggles one row; shift-click extends from the last one
   *  clicked, which is what every list of this shape does. */
  function toggleRow(id: string, shiftKey: boolean) {
    const next = new Set(selected);
    if (shiftKey && lastClicked) {
      const ids = rows.map((r) => r.id);
      const from = ids.indexOf(lastClicked);
      const to = ids.indexOf(id);
      if (from !== -1 && to !== -1) {
        const [lo, hi] = from < to ? [from, to] : [to, from];
        for (const between of ids.slice(lo, hi + 1)) next.add(between);
        selected = next;
        lastClicked = id;
        return;
      }
    }
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selected = next;
    lastClicked = id;
  }

  function clearSelection() {
    selected = new Set();
    lastClicked = null;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && selected.size > 0) clearSelection();
  }

  /** Run an action, report what it did, refresh so the list reflects it
   *  on the click rather than at the next poll. */
  async function act(run: () => Promise<string>) {
    if (busy) return;
    busy = true;
    try {
      actionMsg = await run();
    } catch (e: unknown) {
      actionMsg = e instanceof Error ? e.message : String(e);
      log("error", `Queue action failed: ${actionMsg}`);
    } finally {
      busy = false;
      await refreshQueueSnapshot();
      setTimeout(() => (actionMsg = ""), 3000);
    }
  }

  const plural = (n: number, word: string) => `${fmtNum(n)} ${word}${n === 1 ? "" : "s"}`;

  const startWorkers = () =>
    act(async () =>
      (await api.runQueue()) ? "Workers started" : "Workers already running",
    );

  const retryFailed = () =>
    act(async () => `Requeued ${plural(await api.requeueFailed(), "task")}`);

  const clearFinished = () =>
    act(async () => `Removed ${plural(await api.clearTasks(), "finished task")}`);

  const cancelSelected = () =>
    act(async () => {
      const n = await api.cancelTasks([...selected]);
      clearSelection();
      return `Cancelled ${plural(n, "task")}`;
    });

  const retrySelected = () =>
    act(async () => {
      const n = await api.requeueTasks([...selected]);
      clearSelection();
      return `Requeued ${plural(n, "task")}`;
    });

  const removeSelected = () =>
    act(async () => {
      const n = await api.removeTasks([...selected]);
      clearSelection();
      return `Removed ${plural(n, "task")}`;
    });

  const duplicateSelected = () =>
    act(async () => {
      const n = await api.duplicateTasks([...selected]);
      clearSelection();
      return `Queued ${plural(n, "copy")}`;
    });

  const bumpSelected = () =>
    act(async () => {
      const n = await api.bumpTasks([...selected]);
      return n === 0
        ? "Nothing left to move — already claimed"
        : `Moved ${plural(n, "task")} to the front`;
    });

  const revealTask = (t: TaskView) =>
    act(async () => {
      await revealItemInDir(t.path);
      return "";
    });

  /** Remaining work, which is the only honest thing to say: task
   *  duration varies by symbol and date range, so a time estimate from
   *  a rolling average would be fiction. */
  const remaining = $derived(countOf("pending") + countOf("running"));
</script>

<svelte:window onkeydown={onKeydown} />

<div class="queue-view">
  <!-- Header: what the queue holds, and what to do with all of it -->
  <div class="queue-header">
    <div class="header-left">
      <div class="title-row">
        <h1 class="queue-title">Queue</h1>
        <div class="layout-toggle" role="tablist" aria-label="Queue layout">
          <button role="tab" aria-selected={layout === "transfers"} class:active={layout === "transfers"} onclick={() => (layout = "transfers")}>Downloads</button>
          <button role="tab" aria-selected={layout === "tasks"} class:active={layout === "tasks"} onclick={() => (layout = "tasks")}>Tasks</button>
        </div>
      </div>
      {#if snap}
        <span class="queue-meta text-figures">
          <span>{plural(totalCount, "task")}</span>
          <span class="sep">·</span>
          <span>{fmtBytes(snap.bytes_on_disk)} on disk</span>
          <span class="sep">·</span>
          <span>{fmtNum(snap.files_on_disk)} files</span>
          {#if remaining > 0}
            <span class="sep">·</span>
            <span>{fmtNum(remaining)} remaining</span>
          {/if}
        </span>
      {/if}
    </div>

    <div class="header-actions">
      {#if actionMsg}
        <span class="action-feedback text-body-sm" role="status">{actionMsg}</span>
      {/if}
      <button class="btn btn-primary" onclick={startWorkers} disabled={busy || remaining === 0}>
        <Play size={13} strokeWidth={1.75} />
        Start workers
      </button>
      {#if countOf("failed") > 0}
        <button class="btn btn-secondary" onclick={retryFailed} disabled={busy}>
          <RotateCcw size={13} strokeWidth={1.75} />
          Retry failed ({fmtNum(countOf("failed"))})
        </button>
      {/if}
      {#if finishedCount > 0}
        <button class="btn btn-secondary" onclick={clearFinished} disabled={busy}>
          <Trash2 size={13} strokeWidth={1.75} />
          Clear finished ({fmtNum(finishedCount)})
        </button>
      {/if}
    </div>
  </div>

  {#if layout === "transfers"}
    <div class="transfers-body">
      <TransfersList />
    </div>
  {:else}
  <!-- Filter row -->
  <div class="filter-bar">
    <div class="filter-pills" role="group" aria-label="Status filter">
      {#each FILTERS as f}
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
          <span class="filter-count text-figures">{fmtNum(countOf(f))}</span>
        </button>
      {/each}
    </div>

    <div class="search-wrap">
      <Search size={14} strokeWidth={1.75} class="search-icon" aria-hidden="true" />
      <input
        class="search-input"
        type="search"
        placeholder="Filter symbol, dataset or date…"
        bind:value={query}
        aria-label="Filter tasks"
      />
    </div>
  </div>

  <!-- Selection action bar. Present only when something is selected. -->
  {#if selected.size > 0}
    <div class="bulk-bar" role="region" aria-label="Actions for selected tasks">
      <span class="bulk-count">{plural(selected.size, "task")} selected</span>
      <div class="bulk-actions">
        <button class="btn btn-secondary" onclick={bumpSelected} disabled={busy}>
          <ArrowUp size={13} strokeWidth={1.75} /> Move to front
        </button>
        <button class="btn btn-secondary" onclick={duplicateSelected} disabled={busy}>
          <Copy size={13} strokeWidth={1.75} /> Duplicate
        </button>
        <button class="btn btn-secondary" onclick={retrySelected} disabled={busy}>
          <RotateCcw size={13} strokeWidth={1.75} /> Retry
        </button>
        <button class="btn btn-secondary" onclick={cancelSelected} disabled={busy || !canCancel}>
          <X size={13} strokeWidth={1.75} /> Cancel
        </button>
        <button
          class="btn btn-secondary danger"
          onclick={removeSelected}
          disabled={busy}
          title="Delete these tasks from the queue. Anything still running stops being tracked."
        >
          <Trash2 size={13} strokeWidth={1.75} /> Remove
        </button>
      </div>
      <button class="btn btn-ghost" onclick={clearSelection}>Clear selection</button>
    </div>
  {/if}

  <!-- Rows -->
  <div class="task-list">
    {#if rows.length > 0}
      <div class="list-head">
        <label class="select-cell">
          <input
            type="checkbox"
            checked={allVisibleSelected}
            indeterminate={someVisibleSelected}
            onchange={toggleAllVisible}
            aria-label="Select all visible tasks"
          />
        </label>
        <span class="head-label text-caption">
          {plural(rows.length, "task")} shown
          {#if totalCount > rows.length}
            <span class="fg-subtle">of {fmtNum(totalCount)}</span>
          {/if}
        </span>
      </div>
    {/if}

    <!-- An empty list needs an explanation whatever the reason for it.
         This was gated on being *disconnected*, so the one state a
         working user actually reaches — connected, nothing queued yet —
         rendered a blank panel under the filter bar. -->
    {#if rows.length === 0}
      <div class="empty-queue">
        <div class="empty-icon" aria-hidden="true">
          <svg width="40" height="40" viewBox="0 0 40 40" fill="none">
            <rect x="6" y="6" width="28" height="28" rx="8" stroke="var(--border-strong)" stroke-width="1.5" />
            <path d="M14 20h12M20 14v12" stroke="var(--fg-subtle)" stroke-width="1.5" stroke-linecap="round" />
          </svg>
        </div>
        {#if app.connState !== "connected"}
          <p class="empty-label">Not connected</p>
          <p class="empty-sub text-body-sm fg-muted">
            Sign in from Settings to open the queue.
          </p>
        {:else if totalCount > 0}
          <p class="empty-label">Nothing matches</p>
          <p class="empty-sub text-body-sm fg-muted">
            {plural(totalCount, "task")} in the queue, none in this view.
          </p>
          <button class="btn btn-secondary btn-sm" onclick={clearFilters}>
            Clear filters
          </button>
        {:else}
          <p class="empty-label">Queue is empty</p>
          <p class="empty-sub text-body-sm fg-muted">
            Pick a dataset in Browse and queue it — tasks show up here as
            they run.
          </p>
          <button class="btn btn-secondary btn-sm" onclick={() => navigate("browse")}>
            Browse datasets
            <ArrowRight size={14} strokeWidth={1.75} />
          </button>
        {/if}
      </div>
    {:else}
      {#each rows as task (task.id)}
        <div
          class="task-row"
          class:row-running={task.status === "running"}
          class:row-selected={selected.has(task.id)}
        >
          <label class="select-cell">
            <input
              type="checkbox"
              checked={selected.has(task.id)}
              onclick={(e) => toggleRow(task.id, e.shiftKey)}
              aria-label="Select {task.symbol} {task.date}"
            />
          </label>

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

          <div class="task-main">
            <div class="task-top">
              <code class="task-kind">{task.kind}</code>
              <span class="task-symbol">{task.symbol}</span>
              <span class="task-date text-figures">
                {task.date}{task.end_date ? ` → ${task.end_date}` : ""}
              </span>
            </div>

            {#if task.status === "running"}
              <!--
                Indeterminate progress: the SDK does not surface row-level
                progress mid-call, so a percentage would be a lie. The
                shimmer says "in flight, still alive" and nothing more.
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

          <div class="task-stats">
            {#if task.rows != null}
              <span class="stat text-figures">{fmtNum(task.rows)} rows</span>
            {/if}
            {#if task.bytes != null}
              <span class="stat text-figures">{fmtBytes(task.bytes)}</span>
            {/if}
            {#if task.attempts > 1}
              <span class="stat attempts">attempt {task.attempts}</span>
            {/if}
          </div>

          <div class="task-actions" aria-label="Row actions">
            {#if task.status === "pending"}
              <button
                class="btn-icon"
                onclick={() => act(async () => {
                  const n = await api.bumpTasks([task.id]);
                  return n ? `${task.symbol} ${task.date} moved to the front` : "Already claimed by a worker";
                })}
                title="Move to the front of the queue"
                aria-label="Move {task.symbol} {task.date} to the front of the queue"
              >
                <ArrowUp size={13} strokeWidth={1.75} />
              </button>
            {/if}
            <button
              class="btn-icon"
              onclick={() => act(async () => {
                await api.duplicateTasks([task.id]);
                return `Queued another ${task.symbol} ${task.date}`;
              })}
              title="Queue another copy"
              aria-label="Queue another copy of {task.symbol} {task.date}"
            >
              <Copy size={13} strokeWidth={1.75} />
            </button>
            {#if task.status === "done"}
              <button
                class="btn-icon"
                onclick={() => revealTask(task)}
                title="Show the file"
                aria-label="Show the file for {task.symbol} {task.date}"
              >
                <FolderOpen size={13} strokeWidth={1.75} />
              </button>
            {/if}
            {#if task.status === "pending" || task.status === "running"}
              <button
                class="btn-icon danger"
                onclick={() => act(async () => {
                  await api.cancelTasks([task.id]);
                  return `Cancelled ${task.symbol} ${task.date}`;
                })}
                title="Stop this task, keep the row"
                aria-label="Cancel {task.symbol} {task.date}"
              >
                <X size={13} strokeWidth={1.75} />
              </button>
            {/if}
            <button
              class="btn-icon danger"
              onclick={() => act(async () => {
                await api.removeTasks([task.id]);
                return `Removed ${task.symbol} ${task.date}`;
              })}
              title="Delete this task from the queue"
              aria-label="Remove {task.symbol} {task.date} from the queue"
            >
              <Trash2 size={13} strokeWidth={1.75} />
            </button>
          </div>
        </div>
      {/each}

      {#if rows.length === 0 && totalCount > 0}
        <div class="empty-filter">
          <p class="text-body-sm fg-muted">
            No tasks match {activeFilter === "all" ? "" : STATUS_LABELS[activeFilter].toLowerCase()}
            {query ? `"${query}"` : ""}.
          </p>
        </div>
      {/if}
    {/if}
  </div>
  {/if}
</div>

<style>
  .title-row { display: flex; align-items: center; gap: var(--sp-4); }
  .layout-toggle {
    display: inline-flex;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 2px;
  }
  .layout-toggle button {
    border: none;
    background: none;
    padding: 3px 10px;
    border-radius: calc(var(--r-sm) - 2px);
    font: inherit;
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
    cursor: pointer;
  }
  .layout-toggle button.active {
    background: var(--surface-1);
    color: var(--fg);
    box-shadow: 0 0 0 1px var(--border);
  }
  .transfers-body { flex: 1; overflow-y: auto; }
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
    justify-content: space-between;
    gap: var(--sp-4);
    padding: var(--sp-3) var(--sp-8);
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
  }
  .filter-pills {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    overflow-x: auto;
    scrollbar-width: none;
  }
  .filter-pills::-webkit-scrollbar { display: none; }

  .search-wrap {
    position: relative;
    display: flex;
    align-items: center;
    flex-shrink: 0;
    width: 260px;
  }
  .search-wrap :global(.search-icon) {
    position: absolute;
    left: var(--sp-2);
    color: var(--fg-subtle);
    pointer-events: none;
  }
  .search-input {
    width: 100%;
    height: 30px;
    padding: 0 var(--sp-2) 0 var(--sp-6);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    color: var(--fg);
    font-size: var(--text-body-sm);
    outline: none;
  }
  .search-input:focus-visible {
    border-color: var(--accent);
    box-shadow: var(--shadow-glow-accent);
  }
  .search-input::placeholder { color: var(--fg-subtle); }

  /* Selection action bar. Only mounted when something is selected, so
     the resting list stays quiet. */
  .bulk-bar {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-2) var(--sp-8);
    flex-shrink: 0;
    background: var(--accent-tint);
    border-bottom: 1px solid var(--accent-ring);
  }
  .bulk-count {
    font-size: var(--text-body-sm);
    font-weight: var(--weight-semi);
    color: var(--fg);
    white-space: nowrap;
  }
  .bulk-actions {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }
  .bulk-bar .btn-ghost { margin-left: auto; }
  .btn.danger { color: var(--bad); }
  .btn.danger:hover:not(:disabled) { border-color: var(--bad-ring); background: var(--bad-tint); }

  /* Header strip above the rows: the select-all box lines up with the
     per-row boxes below it. */
  .list-head {
    display: grid;
    grid-template-columns: 20px 1fr;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-2) var(--sp-8);
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    background: var(--bg);
    z-index: 1;
  }
  .head-label { color: var(--fg-muted); }

  .select-cell {
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
  }
  .select-cell input {
    accent-color: var(--accent);
    cursor: pointer;
    margin: 0;
  }

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
  .filter-pill.active.filter-done    { color: var(--good); border-color: var(--good-tint); background: var(--good-tint); }
  .filter-pill.active.filter-failed  { color: var(--bad);  border-color: var(--bad-tint); background: var(--bad-tint); }
  .filter-pill.active.filter-empty   { color: var(--warn); border-color: var(--warn-tint); background: var(--warn-tint); }

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
    grid-template-columns: 20px 20px 1fr auto auto;
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
    background: var(--accent-tint-weak);
  }
  .task-row.row-selected {
    background: var(--accent-tint);
  }
  .task-row.row-selected:hover { background: var(--accent-tint-strong); }

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
    font-size: var(--text-figures);
    color: var(--fg-muted);
    background: var(--surface-2);
    padding: 1px var(--sp-2);
    border-radius: var(--r-sm);
  }

  .task-symbol {
    font-weight: var(--weight-semi);
    font-family: var(--font-mono);
    font-size: var(--text-figures);
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

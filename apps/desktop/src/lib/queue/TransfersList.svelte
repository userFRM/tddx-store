<script lang="ts">
  /**
   * Downloads as the user asked for them — one row per request, the way
   * a torrent client shows one row per transfer.
   *
   * The app splits requests into tasks for the server's sake (windows no
   * wider than a year, one task per expiration where the wildcard is
   * refused, one per trading day for single-date endpoints). That is
   * right, and it is not what anyone asked for. This rolls the tasks
   * back up into the thing they did ask for.
   *
   * Progress here is real. The server reports nothing mid-request, so a
   * single task cannot show a percentage honestly; a download made of
   * many tasks can, as tasks finished over tasks planned.
   */
  import {
    Pause,
    Play,
    Trash2,
    FolderOpen,
    CheckCircle2,
    XCircle,
    Loader2,
    Clock,
    PauseCircle,
    Download,
  } from "lucide-svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api, fmtBytes, fmtNum, type Batch } from "$lib/api";
  import { app, log, navigate, refreshQueueSnapshot } from "$lib/stores/app.svelte";

  let busyId = $state<string | null>(null);

  type Phase = "downloading" | "queued" | "paused" | "done" | "partial" | "failed";

  function effectiveTotal(b: Batch): number {
    return Math.max(0, b.total - b.split);
  }

  function finished(b: Batch): number {
    return b.done + b.empty + b.failed;
  }

  function phase(b: Batch): Phase {
    if (b.running > 0) return "downloading";
    if (b.pending > 0) return "queued";
    if (b.paused > 0) return "paused";
    if (b.failed > 0) return b.done + b.empty > 0 ? "partial" : "failed";
    return "done";
  }

  function progress(b: Batch): number {
    const total = effectiveTotal(b);
    return total === 0 ? 0 : finished(b) / total;
  }

  function label(b: Batch): string {
    const entry = app.catalogue.find((e) => e.name === b.kind);
    const dataset = entry?.summary || b.kind;
    const who = b.symbols > 1 ? `${b.first_symbol} +${b.symbols - 1}` : b.first_symbol;
    return `${who} · ${dataset}`;
  }

  function ymd(s: string): string {
    return s.length === 8 ? `${s.slice(0, 4)}-${s.slice(4, 6)}-${s.slice(6, 8)}` : s;
  }

  /** How many workers a batch can use right now: the plan's budget,
   *  lowered to the user's cap. */
  const workers = $derived(
    Math.max(
      1,
      Math.min(
        app.tierStatus?.in_flight_budget ?? 1,
        app.settings.preferences?.max_concurrency ?? Number.POSITIVE_INFINITY,
      ),
    ),
  );

  /** Remaining tasks × measured mean duration, spread across the workers
   *  that can take them. Measured on this batch, so it reflects this
   *  dataset at this size rather than a global average. */
  function eta(b: Batch): number | null {
    const remaining = b.pending + b.running;
    if (remaining === 0 || !b.avg_task_secs) return null;
    return (remaining * b.avg_task_secs) / Math.max(1, Math.min(workers, remaining));
  }

  function fmtDuration(s: number): string {
    if (s < 60) return `${Math.max(1, Math.round(s))}s`;
    if (s < 3600) return `${Math.floor(s / 60)}m ${Math.round(s % 60)}s`;
    return `${Math.floor(s / 3600)}h ${Math.round((s % 3600) / 60)}m`;
  }

  /** Speed over the time the download was actually running — from the
   *  first task picked up, not from when it was queued, so time spent
   *  waiting in line or paused is not reported as slowness. */
  function avgRate(b: Batch): string | null {
    if (!b.finished_at || !b.started_at || b.bytes === 0) return null;
    const secs = Math.max(1, b.finished_at - b.started_at);
    return `${fmtBytes(b.bytes / secs)}/s avg`;
  }

  async function act(b: Batch, what: "pause" | "resume" | "remove") {
    busyId = b.id;
    try {
      if (what === "pause") await api.pauseBatch(b.id);
      else if (what === "resume") {
        await api.resumeBatch(b.id);
        // A resumed download needs workers; start them if none are live.
        await api.runQueue().catch(() => false);
      } else await api.removeBatch(b.id);
      await refreshQueueSnapshot();
    } catch (e: unknown) {
      log("error", `Could not ${what} download: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      busyId = null;
    }
  }

  async function reveal(b: Batch) {
    try {
      await revealItemInDir(`${app.settings.output_dir}/${b.kind}`);
    } catch (e: unknown) {
      log("error", `Could not open folder: ${e instanceof Error ? e.message : String(e)}`);
    }
  }
</script>

{#if app.batches.length === 0}
  <div class="empty">
    <Download size={28} strokeWidth={1.5} />
    <p class="empty-label">No downloads yet</p>
    <p class="text-body-sm fg-muted">Pick a dataset in Browse and queue it. Each request shows up here as one download.</p>
    <button class="btn btn-secondary btn-sm" onclick={() => navigate("browse")}>Browse datasets</button>
  </div>
{:else}
  <ul class="transfers" aria-label="Downloads">
    {#each app.batches as b (b.id)}
      {@const p = phase(b)}
      {@const pct = progress(b)}
      {@const remaining = eta(b)}
      <li class="transfer" class:is-paused={p === "paused"} class:is-failed={p === "failed"}>
        <span class="state" aria-hidden="true">
          {#if p === "downloading"}<Loader2 size={16} class="spin" />
          {:else if p === "queued"}<Clock size={16} />
          {:else if p === "paused"}<PauseCircle size={16} />
          {:else if p === "done"}<CheckCircle2 size={16} />
          {:else}<XCircle size={16} />{/if}
        </span>

        <div class="main">
          <div class="title-row">
            <span class="title">{label(b)}</span>
            <span class="window tabnum">
              {ymd(b.start)}{b.end && b.end !== b.start ? ` → ${ymd(b.end)}` : ""} · {b.format}
            </span>
          </div>
          <div
            class="bar"
            role="progressbar"
            aria-valuemin="0"
            aria-valuemax="100"
            aria-valuenow={Math.round(pct * 100)}
            aria-label="{label(b)} progress"
          >
            <div class="fill" class:done={p === "done"} class:bad={p === "failed" || p === "partial"} style:width="{pct * 100}%"></div>
          </div>
          <div class="meta tabnum">
            <span>{fmtNum(finished(b))} of {fmtNum(effectiveTotal(b))}</span>
            <span class="sep">·</span>
            <span>{fmtBytes(b.bytes)}</span>
            {#if b.rows > 0}<span class="sep">·</span><span>{fmtNum(b.rows)} rows</span>{/if}
            {#if b.failed > 0}<span class="sep">·</span><span class="bad-text">{b.failed} failed</span>{/if}
            <span class="spacer"></span>
            {#if p === "downloading" || p === "queued"}
              <span class="fg-muted">{remaining !== null ? `${fmtDuration(remaining)} left` : "estimating…"}</span>
            {:else if p === "paused"}
              <span class="fg-muted">Paused</span>
            {:else if avgRate(b)}
              <span class="fg-muted">{avgRate(b)}</span>
            {/if}
          </div>
        </div>

        <div class="actions">
          {#if p === "downloading" || p === "queued"}
            <button class="btn-icon" title="Pause" aria-label="Pause {label(b)}" disabled={busyId === b.id} onclick={() => act(b, "pause")}>
              <Pause size={14} />
            </button>
          {:else if p === "paused"}
            <button class="btn-icon" title="Resume" aria-label="Resume {label(b)}" disabled={busyId === b.id} onclick={() => act(b, "resume")}>
              <Play size={14} />
            </button>
          {/if}
          <button class="btn-icon" title="Show in folder" aria-label="Show {label(b)} in folder" onclick={() => reveal(b)}>
            <FolderOpen size={14} />
          </button>
          <button class="btn-icon" title="Remove from list" aria-label="Remove {label(b)}" disabled={busyId === b.id} onclick={() => act(b, "remove")}>
            <Trash2 size={14} />
          </button>
        </div>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .transfers { list-style: none; margin: 0; padding: var(--sp-2) 0; }
  .transfer {
    display: grid;
    grid-template-columns: 20px 1fr auto;
    gap: var(--sp-3);
    align-items: center;
    padding: var(--sp-3) var(--sp-8);
    border-bottom: 1px solid var(--border);
  }
  .transfer:hover { background: var(--surface-2); }
  .state { color: var(--accent); display: inline-flex; }
  .is-paused .state { color: var(--fg-subtle); }
  .is-failed .state { color: var(--bad); }
  .main { min-width: 0; display: flex; flex-direction: column; gap: 6px; }
  .title-row { display: flex; align-items: baseline; gap: var(--sp-3); min-width: 0; }
  .title { font-weight: var(--weight-semi); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .window { color: var(--fg-subtle); font-size: var(--text-caption); white-space: nowrap; }
  .bar { height: 4px; border-radius: 2px; background: var(--surface-3); overflow: hidden; }
  .fill { height: 100%; background: var(--accent-fill); transition: width var(--dur-base) var(--ease-standard); }
  .fill.done { background: var(--good); }
  .fill.bad { background: var(--bad); }
  .meta { display: flex; gap: 6px; align-items: center; font-size: var(--text-caption); color: var(--fg-muted); }
  .sep { color: var(--fg-subtle); }
  .spacer { flex: 1; }
  .bad-text { color: var(--bad); }
  .actions { display: flex; gap: 2px; }
  .empty {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: var(--sp-3); min-height: 300px; text-align: center; padding: var(--sp-8);
    color: var(--fg-subtle);
  }
  .empty-label { margin: 0; color: var(--fg); font-weight: var(--weight-semi); }
  .empty p { margin: 0; max-width: 42ch; }
</style>

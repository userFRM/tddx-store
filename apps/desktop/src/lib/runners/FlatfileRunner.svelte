<script lang="ts">
  /**
   * Queue whole-market flat files for a range of trading days.
   *
   * This used to fetch one day straight to a path and forget it: no
   * progress, no retry, nothing in the Library, and no way to schedule
   * it. Each served flat file is now a dataset kind, so a range of days
   * becomes queue tasks like any other download — one per trading day,
   * visible in Downloads, pausable, retried on failure, filed where the
   * Library looks.
   */
  import { X, Loader2, FileArchive, ListPlus, ArrowRight } from "lucide-svelte";
  import { app, log, navigate, refreshQueueSnapshot } from "$lib/stores/app.svelte";
  import {
    api,
    WHOLE_MARKET,
    type EnqueuePlan,
    type FlatfileReqType,
    type FlatfileSecType,
  } from "$lib/api";

  type FF = {
    title: string;
    sec: FlatfileSecType;
    req: FlatfileReqType;
    desc: string;
  };

  function close() {
    app.flatfileRunnerOpen = false;
    app.endpointRunner = null;
    queued = 0;
    msg = "";
  }

  const ff = $derived(
    (app.endpointRunner as unknown as { _flatfile?: FF } | null)?._flatfile ?? null,
  );
  const kind = $derived(ff ? `flatfile_${ff.sec.toLowerCase()}_${ff.req}` : "");

  /** The last settled session: yesterday, walked back over a weekend. */
  function lastSession(): string {
    const d = new Date();
    d.setDate(d.getDate() - 1);
    while (d.getDay() === 0 || d.getDay() === 6) d.setDate(d.getDate() - 1);
    return d.toISOString().slice(0, 10);
  }

  let start = $state(lastSession());
  let end = $state(lastSession());
  let format = $state<"csv" | "jsonl">("csv");
  let busy = $state(false);
  let msg = $state("");
  let queued = $state(0);
  let plan = $state<EnqueuePlan | null>(null);

  const ymd = (iso: string) => iso.replace(/-/g, "");

  function args(batch_id?: string) {
    return {
      kind,
      symbol: WHOLE_MARKET,
      format,
      start: ymd(start),
      end: ymd(end),
      batch_id: batch_id ?? null,
    };
  }

  // How many archives the range is, from the same planner that queues.
  $effect(() => {
    void [kind, start, end, format];
    if (!app.flatfileRunnerOpen || !kind || !start || !end || end < start) {
      plan = null;
      return;
    }
    let cancelled = false;
    const t = setTimeout(() => {
      api
        .estimate(args())
        .then((p) => !cancelled && (plan = p))
        .catch(() => !cancelled && (plan = null));
    }, 300);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  });

  async function queue() {
    if (!ff) return;
    if (end < start) {
      msg = "The end date is before the start date.";
      return;
    }
    busy = true;
    msg = "";
    try {
      queued = await api.enqueue(args(crypto.randomUUID()));
      await api.runQueue().catch(() => false);
      await refreshQueueSnapshot();
      log("info", `Queued ${queued} ${ff.title} archive${queued === 1 ? "" : "s"}`);
    } catch (e: unknown) {
      msg = e instanceof Error ? e.message : String(e);
      log("error", `Flat file queue failed: ${msg}`);
    } finally {
      busy = false;
    }
  }

  function viewDownloads() {
    close();
    navigate("queue");
  }
</script>

{#if app.flatfileRunnerOpen && ff}
  <div class="backdrop" onclick={close} role="presentation">
    <div class="card" onclick={(e) => e.stopPropagation()}
         role="dialog" aria-modal="true" tabindex="-1"
         onkeydown={(e) => e.key === "Escape" && close()}>
      <header class="head">
        <div>
          <span class="text-caption">Flat file · whole market, one archive per day</span>
          <h2 class="title"><FileArchive size={18} /> {ff.title}</h2>
          <p class="sub fg-muted">{ff.desc}</p>
        </div>
        <button class="btn-icon" onclick={close} aria-label="Close"><X size={14} /></button>
      </header>

      <div class="form">
        <div class="row">
          <label class="field">
            <span class="text-caption">From</span>
            <input class="field-input text-figures" type="date" bind:value={start} />
          </label>
          <label class="field">
            <span class="text-caption">To</span>
            <input class="field-input text-figures" type="date" bind:value={end} />
          </label>
        </div>
        <label class="field">
          <span class="text-caption">Format</span>
          <select class="field-input" bind:value={format}>
            <option value="csv">CSV</option>
            <option value="jsonl">JSON Lines</option>
          </select>
        </label>
        <p class="hint fg-muted">
          {#if plan}
            {plan.requests.toLocaleString()} trading day{plan.requests === 1 ? "" : "s"} —
            one archive each, saved under <code>{kind}</code> in your library.
          {:else}
            Each trading day is one archive, often several gigabytes.
          {/if}
        </p>
      </div>

      <footer class="foot">
        <span class="msg" class:error={!!msg}>{msg}</span>
        {#if queued > 0}
          <button class="btn btn-primary" onclick={viewDownloads}>
            Queued {queued} — view downloads <ArrowRight size={14} />
          </button>
        {:else}
          <button class="btn btn-primary" onclick={queue} disabled={busy || !start || !end}>
            {#if busy}<Loader2 class="spin" size={14} />Queueing…
            {:else}<ListPlus size={14} />Queue {plan ? plan.requests.toLocaleString() : ""} archive{plan?.requests === 1 ? "" : "s"}
            {/if}
          </button>
        {/if}
      </footer>
    </div>
  </div>
{/if}

<style>
  .head .text-caption { display: block; margin-bottom: 4px; }
  .hint { font-size: var(--text-body-sm); margin: 0; }
  .backdrop {
    position: fixed; inset: 0;
    background: var(--scrim);
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

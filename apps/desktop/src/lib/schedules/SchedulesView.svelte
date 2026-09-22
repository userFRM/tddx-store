<script lang="ts">
  /**
   * Cron-style scheduler: persistent recurring downloads. Stored in the
   * SQLite queue DB alongside the queue itself. The desktop app's
   * runtime ticks each schedule on startup; firing each row enqueues a
   * fresh task for "yesterday" with the schedule's kind / symbol /
   * format.
   */
  import { onMount, onDestroy } from "svelte";
  import {
    Plus,
    Trash2,
    Pause,
    Play,
    CalendarClock,
    Loader2,
  } from "lucide-svelte";
  import { api, TAURI_AVAILABLE, type ScheduleRow } from "$lib/api";
  import { app, log } from "$lib/stores/app.svelte";

  let rows = $state<ScheduleRow[]>([]);
  let loading = $state(false);
  let creating = $state(false);
  /** The dropdown offered seven hardcoded legacy names, so 53 of the
   *  catalogue's datasets could not be scheduled at all. Driven off the
   *  catalogue, grouped by asset class. */
  const datasetGroups = $derived.by(() => {
    const order: string[] = [];
    const map = new Map<string, typeof app.catalogue>();
    for (const entry of app.catalogue) {
      if (entry.subcategory === "list") continue; // nothing to download
      const cat = entry.category.charAt(0).toUpperCase() + entry.category.slice(1);
      if (!map.has(cat)) {
        map.set(cat, []);
        order.push(cat);
      }
      map.get(cat)!.push(entry);
    }
    return order.map((category) => ({ category, entries: map.get(category)! }));
  });

  let composer = $state({
    name: "",
    kind: "stock_history_trade_quote",
    symbol: "",
    format: "parquet",
    cron_kind: "weekdays",
    at_time: "17:30",
  });
  let msg = $state("");

  let timer: ReturnType<typeof setInterval> | null = null;

  async function refresh() {
    if (!TAURI_AVAILABLE) return;
    loading = true;
    try {
      rows = await api.scheduleList();
    } catch (e: unknown) {
      msg = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }
  onMount(() => {
    refresh();
    timer = setInterval(refresh, 10_000);
  });
  onDestroy(() => timer && clearInterval(timer));

  async function create() {
    if (!composer.name || !composer.symbol) {
      msg = "Name and symbol required";
      return;
    }
    creating = true;
    msg = "Creating…";
    try {
      const s = await api.scheduleCreate(composer);
      rows = [s, ...rows];
      log("info", `Created schedule ${s.name}`, { id: s.id, cron: s.cron_kind });
      composer = { ...composer, name: "", symbol: "" };
      msg = "Created";
      setTimeout(() => (msg = ""), 1500);
    } catch (e: unknown) {
      msg = e instanceof Error ? e.message : String(e);
    } finally {
      creating = false;
    }
  }

  async function remove(id: string) {
    try {
      await api.scheduleDelete(id);
      rows = rows.filter((r) => r.id !== id);
      log("info", "Deleted schedule", { id });
    } catch (e: unknown) {
      msg = e instanceof Error ? e.message : String(e);
    }
  }

  async function togglePause(r: ScheduleRow) {
    try {
      await api.scheduleSetPaused(r.id, !r.paused);
      rows = rows.map((x) => (x.id === r.id ? { ...x, paused: !x.paused } : x));
    } catch (e: unknown) {
      msg = e instanceof Error ? e.message : String(e);
    }
  }

  function fmtLast(t: number | null): string {
    if (t === null) return "never";
    return new Date(t * 1000).toLocaleString();
  }
</script>

<div class="schedules-view">
  <header>
    <span class="text-caption">Library · Scheduler</span>
    <h1 class="title">Recurring downloads</h1>
    <p class="sub fg-muted">
      A schedule queues one dataset for one symbol on the days you pick,
      covering the previous session. The app checks every minute while it
      is open; times are your machine's, so leave headroom after the
      close rather than scheduling on the boundary.
    </p>
    <p class="sub fg-subtle">
      Whole-market flat-file archives are not schedulable yet — those
      download outside the queue. Track it in
      <a
        href="https://github.com/userFRM/tddx-store/issues/3"
        target="_blank"
        rel="noreferrer">issue #3</a
      >.
    </p>
  </header>

  <section class="card composer-card">
    <div class="composer-row">
      <label class="field">
        <span class="text-caption">Name</span>
        <input class="field-input" bind:value={composer.name} placeholder="QQQ EOD daily" />
      </label>
      <label class="field">
        <span class="text-caption">Symbol</span>
        <input class="field-input text-figures" bind:value={composer.symbol} placeholder="QQQ" />
      </label>
      <label class="field">
        <span class="text-caption">Dataset</span>
        <select class="field-input" bind:value={composer.kind}>
          {#each datasetGroups as group (group.category)}
            <optgroup label={group.category}>
              {#each group.entries as entry (entry.name)}
                <option value={entry.name}>{entry.summary || entry.name}</option>
              {/each}
            </optgroup>
          {/each}
        </select>
      </label>
    </div>
    <div class="composer-row">
      <label class="field">
        <span class="text-caption">Format</span>
        <select class="field-input" bind:value={composer.format}>
          <option value="parquet">Parquet</option>
          <option value="csv">CSV</option>
          <option value="jsonl">JSON Lines</option>
          <option value="json">JSON</option>
        </select>
      </label>
      <label class="field">
        <span class="text-caption">Cadence</span>
        <select class="field-input" bind:value={composer.cron_kind}>
          <option value="daily">Daily</option>
          <option value="weekdays">Weekdays</option>
          <option value="weekly:mon">Weekly · Mon</option>
          <option value="weekly:fri">Weekly · Fri</option>
        </select>
      </label>
      <label class="field">
        <span class="text-caption">Fire at (HH:MM, your timezone)</span>
        <input class="field-input tabnum" bind:value={composer.at_time} placeholder="17:30" />
      </label>
      <button class="btn btn-primary" onclick={create} disabled={creating}>
        {#if creating}<Loader2 class="spin" size={14} />{:else}<Plus size={14} />{/if}
        Add schedule
      </button>
    </div>
    {#if msg}<div class="msg fg-muted">{msg}</div>{/if}
  </section>

  <section class="list">
    <div class="list-head">
      <span class="text-caption">Active schedules ({rows.length})</span>
      {#if loading}<Loader2 class="spin" size={12} />{/if}
    </div>
    {#if rows.length === 0}
      <div class="empty fg-muted">
        <CalendarClock size={20} />
        <p>No schedules yet. Add one above.</p>
      </div>
    {:else}
      <table class="grid">
        <thead><tr>
          <th>Status</th><th>Name</th><th>Kind</th><th>Symbol</th><th>Cadence</th>
          <th>At</th><th>Last fired</th><th>Format</th><th></th>
        </tr></thead>
        <tbody>
          {#each rows as r (r.id)}
            <tr class:paused={r.paused}>
              <td>
                <span class="pill {r.paused ? 'pill-empty' : 'pill-running'}">
                  <span class="pill-dot"></span>
                  {r.paused ? "Paused" : "Active"}
                </span>
              </td>
              <td class="name-cell">{r.name}</td>
              <td><code>{r.kind}</code></td>
              <td><strong>{r.symbol}</strong></td>
              <td>{r.cron_kind}</td>
              <td class="tabnum">{r.at_time}</td>
              <td class="tabnum fg-muted">{fmtLast(r.last_fired_at)}</td>
              <td><code class="fg-muted">{r.format}</code></td>
              <td class="actions">
                <button class="btn-icon" title={r.paused ? "Resume" : "Pause"}
                        onclick={() => togglePause(r)}>
                  {#if r.paused}<Play size={12} fill="currentColor" />{:else}<Pause size={12} />{/if}
                </button>
                <button class="btn-icon danger" title="Delete" onclick={() => remove(r.id)}>
                  <Trash2 size={12} />
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>
</div>

<style>
  .schedules-view {
    padding: var(--sp-8);
    overflow-y: auto;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
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

  .card {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    padding: var(--sp-4);
  }
  .composer-card { display: flex; flex-direction: column; gap: var(--sp-3); }
  .composer-row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: var(--sp-3);
    align-items: end;
  }
  .field { display: flex; flex-direction: column; gap: 4px; }
  .msg { font-size: var(--text-body-sm); }

  .list {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .list-head {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }
  .empty {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-6);
    border: 1px dashed var(--border);
    border-radius: var(--r-md);
    background: var(--surface-1);
  }
  .empty p { margin: 0; }

  .grid code {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--fg-muted);
  }
  .name-cell { font-weight: var(--weight-semi); }
  tr.paused td { opacity: 0.55; }

  .actions {
    display: flex;
    gap: 4px;
    justify-content: flex-end;
  }
  .btn-icon.danger:hover { color: var(--bad); }

  :global(.spin) { animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>

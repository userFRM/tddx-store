<script lang="ts">
  /**
   * Diff two coverage snapshots: "what's new on disk between
   * timestamps T1 and T2." Useful for sync audits — show the user
   * exactly what got pulled since their last review.
   *
   * Snapshots are captured client-side via `api.coverage()` and
   * persisted via tauri-plugin-store (kv.ts) on demand (Snapshot
   * button). Lives in the OS-correct app data dir on every platform.
   */
  import { onMount } from "svelte";
  import { Diff, Camera, Trash2, Loader2 } from "lucide-svelte";
  import { api, fmtBytes, fmtNum, type Coverage, TAURI_AVAILABLE } from "$lib/api";
  import { log } from "$lib/stores/app.svelte";
  import { kvGet, kvSet } from "$lib/persistence/kv";

  type Snap = {
    id: string;
    label: string;
    capturedAt: number;
    coverage: Coverage[];
  };

  const KEY = "tdds.coverage_snaps.v1";

  let snaps = $state<Snap[]>([]);
  let leftId = $state<string | null>(null);
  let rightId = $state<string | null>(null);
  let busy = $state(false);

  async function loadSnaps(): Promise<Snap[]> {
    const v = await kvGet<Snap[]>(KEY);
    return Array.isArray(v) ? v : [];
  }
  function persist() {
    void kvSet(KEY, snaps);
  }

  onMount(async () => {
    snaps = await loadSnaps();
    if (snaps.length >= 2) {
      leftId = snaps[snaps.length - 2].id;
      rightId = snaps[snaps.length - 1].id;
    }
  });

  async function capture() {
    if (!TAURI_AVAILABLE) return;
    busy = true;
    try {
      const cov = await api.coverage();
      const snap: Snap = {
        id: crypto.randomUUID(),
        label: new Date().toLocaleString(),
        capturedAt: Date.now(),
        coverage: cov,
      };
      snaps = [...snaps, snap];
      persist();
      rightId = snap.id;
      if (!leftId && snaps.length > 1) leftId = snaps[snaps.length - 2].id;
      log("info", "Coverage snapshot captured", { datasets: cov.length });
    } catch (e: unknown) {
      log("error", `Snapshot failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      busy = false;
    }
  }

  function remove(id: string) {
    snaps = snaps.filter((s) => s.id !== id);
    if (leftId === id) leftId = null;
    if (rightId === id) rightId = null;
    persist();
  }

  type DiffRow = {
    symbol: string;
    kind: string;
    deltaFiles: number;
    deltaBytes: number;
    delta: "added" | "grew" | "shrunk" | "removed" | "unchanged";
  };

  const diff = $derived.by(() => {
    if (!leftId || !rightId) return [] as DiffRow[];
    const left  = snaps.find((s) => s.id === leftId)?.coverage  ?? [];
    const right = snaps.find((s) => s.id === rightId)?.coverage ?? [];
    const key = (c: Coverage) => `${c.symbol}::${c.kind}`;
    const lmap = new Map(left.map((c) => [key(c), c] as const));
    const rmap = new Map(right.map((c) => [key(c), c] as const));
    const out: DiffRow[] = [];
    const seen = new Set<string>();
    for (const k of [...lmap.keys(), ...rmap.keys()]) {
      if (seen.has(k)) continue;
      seen.add(k);
      const l = lmap.get(k);
      const r = rmap.get(k);
      const dF = (r?.files ?? 0) - (l?.files ?? 0);
      const dB = (r?.bytes ?? 0) - (l?.bytes ?? 0);
      let delta: DiffRow["delta"];
      if (!l) delta = "added";
      else if (!r) delta = "removed";
      else if (dF > 0 || dB > 0) delta = "grew";
      else if (dF < 0 || dB < 0) delta = "shrunk";
      else delta = "unchanged";
      out.push({
        symbol: r?.symbol ?? l?.symbol ?? "?",
        kind:   r?.kind   ?? l?.kind   ?? "?",
        deltaFiles: dF,
        deltaBytes: dB,
        delta,
      });
    }
    out.sort((a, b) => Math.abs(b.deltaBytes) - Math.abs(a.deltaBytes));
    return out.filter((r) => r.delta !== "unchanged");
  });
</script>

<div class="coverage-diff">
  <header>
    <div>
      <span class="text-caption">Audit</span>
      <h2 class="title"><Diff size={18} /> Coverage diff</h2>
      <p class="sub fg-muted">
        Compare what's on disk between two snapshots. Capture one before
        a big sync, another after, see exactly what's new.
      </p>
    </div>
    <button class="btn btn-primary" onclick={capture} disabled={busy}>
      {#if busy}<Loader2 size={14} class="spin" />{:else}<Camera size={14} />{/if}
      Snapshot now
    </button>
  </header>

  <section class="snaps">
    <div class="snap-row">
      <label class="snap-pick">
        <span class="text-caption">Before</span>
        <select class="field-input" bind:value={leftId}>
          <option value={null}>—</option>
          {#each snaps as s}
            <option value={s.id}>{s.label}</option>
          {/each}
        </select>
      </label>
      <span class="arrow">→</span>
      <label class="snap-pick">
        <span class="text-caption">After</span>
        <select class="field-input" bind:value={rightId}>
          <option value={null}>—</option>
          {#each snaps as s}
            <option value={s.id}>{s.label}</option>
          {/each}
        </select>
      </label>
    </div>
    {#if snaps.length === 0}
      <p class="hint fg-muted">No snapshots yet. Capture one to start.</p>
    {/if}
    {#if snaps.length > 0}
      <div class="snap-list">
        {#each snaps as s (s.id)}
          <div class="snap-tag">
            <span>{s.label}</span>
            <span class="hint tabnum">{s.coverage.length} datasets</span>
            <button class="btn-icon" onclick={() => remove(s.id)} aria-label="Delete snapshot">
              <Trash2 size={11} />
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  {#if leftId && rightId}
    <section class="diff-table">
      {#if diff.length === 0}
        <p class="empty fg-muted">No changes between these snapshots.</p>
      {:else}
        <table class="grid">
          <thead><tr>
            <th>Δ</th><th>Symbol</th><th>Kind</th>
            <th class="num">Files</th><th class="num">Bytes</th>
          </tr></thead>
          <tbody>
            {#each diff as r}
              <tr class="r-{r.delta}">
                <td><span class="tag tag-{r.delta}">{r.delta}</span></td>
                <td><strong>{r.symbol}</strong></td>
                <td><code>{r.kind}</code></td>
                <td class="num tabnum">{r.deltaFiles >= 0 ? "+" : ""}{fmtNum(r.deltaFiles)}</td>
                <td class="num tabnum">{r.deltaBytes >= 0 ? "+" : "-"}{fmtBytes(Math.abs(r.deltaBytes))}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </section>
  {/if}
</div>

<style>
  .coverage-diff { display: flex; flex-direction: column; gap: var(--sp-4); }
  header { display: flex; justify-content: space-between; align-items: flex-start; gap: var(--sp-3); }
  .title { display: inline-flex; align-items: center; gap: 8px; margin: 0;
           font-family: var(--font-display); font-size: var(--text-display-lg); font-weight: var(--weight-semi); }
  .sub { font-size: var(--text-body-sm); margin: 4px 0 0; max-width: 560px; }

  .snaps { display: flex; flex-direction: column; gap: var(--sp-2); }
  .snap-row { display: flex; align-items: end; gap: var(--sp-3); }
  .snap-pick { display: flex; flex-direction: column; gap: 4px; flex: 1; }
  .arrow { color: var(--fg-muted); padding-bottom: 6px; }
  .snap-list { display: flex; flex-wrap: wrap; gap: 6px; }
  .snap-tag {
    display: inline-flex; align-items: center; gap: 6px;
    padding: 4px var(--sp-2);
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    font-size: var(--text-body-sm);
    color: var(--fg-muted);
  }
  .snap-tag .hint { color: var(--fg-subtle); }

  code { font-family: var(--font-mono); font-size: 11px; color: var(--fg-muted); }
  .tag {
    display: inline-block;
    padding: 1px 6px;
    border-radius: var(--r-pill);
    font-size: 10px;
    font-weight: var(--weight-semi);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .tag-added   { background: rgba(93,212,160,0.18); color: var(--good); }
  .tag-grew    { background: var(--accent-tint); color: var(--accent-hi); }
  .tag-shrunk  { background: rgba(245,197,111,0.18); color: var(--warn); }
  .tag-removed { background: rgba(255,126,126,0.18); color: var(--bad); }

  .empty { padding: var(--sp-4); text-align: center; }
</style>

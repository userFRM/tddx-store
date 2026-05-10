<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { api, fmtBytes, fmtNum, type QueueSnapshot } from "$lib/api";

  let snap: QueueSnapshot | null = $state(null);
  let timer: ReturnType<typeof setInterval>;

  async function refresh() {
    try { snap = await api.snapshot(); } catch (_) { /* not connected yet */ }
  }
  onMount(() => { refresh(); timer = setInterval(refresh, 2000); });
  onDestroy(() => clearInterval(timer));

  const cards = [
    { k: "pending", color: "var(--fg-dim)" },
    { k: "running", color: "var(--accent-hi)" },
    { k: "done", color: "var(--good)" },
    { k: "failed", color: "var(--bad)" },
    { k: "empty", color: "var(--warn)" },
  ];
</script>

<div class="cards">
  {#each cards as c}
    {@const v = snap?.counts.find(([s]) => s === c.k)?.[1] ?? 0}
    <div class="card">
      <div class="k">{c.k}</div>
      <div class="v" style:color={c.color}>{v}</div>
    </div>
  {/each}
  <div class="card">
    <div class="k">on disk</div>
    <div class="v">{snap?.files_on_disk ?? 0} / {fmtBytes(snap?.bytes_on_disk ?? 0)}</div>
  </div>
</div>

<table class="grid">
  <thead><tr>
    <th>status</th><th>kind</th><th>symbol</th><th>date</th>
    <th class="num">rows</th><th class="num">bytes</th><th>error</th>
  </tr></thead>
  <tbody>
    {#each snap?.recent ?? [] as t (t.id)}
      <tr>
        <td><span class="pill pill-{t.status}">{t.status}</span></td>
        <td><code>{t.kind}</code></td>
        <td>{t.symbol}</td>
        <td>{t.date}</td>
        <td class="num">{fmtNum(t.rows)}</td>
        <td class="num">{fmtBytes(t.bytes)}</td>
        <td>{t.error ?? ""}</td>
      </tr>
    {/each}
  </tbody>
</table>

<style>
  .cards {
    display: grid; grid-template-columns: repeat(6, 1fr); gap: 10px; margin-bottom: 14px;
  }
  .cards .card .k {
    color: var(--fg-dim); font-size: 11px; text-transform: uppercase; letter-spacing: 0.5px;
  }
  .cards .card .v {
    font-size: 22px; font-weight: 600; font-family: var(--mono); margin-top: 4px;
  }
  code { font-family: var(--mono); font-size: 12px; color: var(--fg-dim); }
</style>

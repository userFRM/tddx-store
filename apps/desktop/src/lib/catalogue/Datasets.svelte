<script lang="ts">
  import { onMount } from "svelte";
  import { api, fmtBytes, fmtNum, type Coverage } from "$lib/api";

  let rows: Coverage[] = $state([]);
  onMount(async () => { try { rows = await api.coverage(); } catch (_) {} });
</script>

<table class="grid">
  <thead><tr>
    <th>symbol</th><th>kind</th><th class="num">files</th><th class="num">size</th><th>span</th>
  </tr></thead>
  <tbody>
    {#each rows as r}
      <tr>
        <td><strong>{r.symbol}</strong></td>
        <td><code>{r.kind}</code></td>
        <td class="num">{fmtNum(r.files)}</td>
        <td class="num">{fmtBytes(r.bytes)}</td>
        <td>{r.first ?? "—"} → {r.last ?? "—"}</td>
      </tr>
    {/each}
    {#if rows.length === 0}
      <tr><td colspan="5" style="color: var(--fg-dim); text-align: center; padding: 24px;">
        No data yet. Queue a download from the sidebar.
      </td></tr>
    {/if}
  </tbody>
</table>

<style>
  code { font-family: var(--mono); font-size: 12px; color: var(--fg-dim); }
</style>

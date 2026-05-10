<!--
  Subscription-tier indicator. Shows one pill per asset class (Stock,
  Options, Index, Rates) with the user's active tier, and an Upgrade
  button when any class is below Pro. Click the badge to open the
  ThetaData pricing page via tauri-plugin-opener.

  Indices + Rates show "—" with a tooltip until the upstream
  thetadatadx SDK exposes accessor methods for them — the Nexus auth
  response carries the bytes today, just not via the public API.
-->
<script lang="ts">
  import { ShieldCheck, ShieldAlert, ArrowUpRight } from "lucide-svelte";
  import { app } from "$lib/stores/app.svelte";
  import { type TierName } from "$lib/api";
  import { openUrl } from "@tauri-apps/plugin-opener";

  // Class list + tier names + upgrade gating all come from the
  // backend's `tier_status.classes`. The badge does no class-name or
  // tier math; that all lives in `tdds_core::tier`.
  const status = $derived(app.tierStatus);
  const showUpgrade = $derived(!!status && status.classes.some((c) => !c.at_max));

  /** Three-letter pill label so the topbar stays compact. The backend
   *  supplies the long form (`Stocks`, `Options`, …) for full-card
   *  contexts; the topbar just abbreviates. Keep this list local —
   *  it's display chrome, not domain state. */
  const SHORT_LABEL: Record<string, string> = {
    stock: "Stock",
    option: "Opt",
    index: "Idx",
    rate: "Rates",
  };

  function tierClass(tier: TierName): string {
    if (tier === "Pro") return "pill pro";
    if (tier === "Standard") return "pill standard";
    if (tier === "Value") return "pill value";
    return "pill free";
  }

  async function handleUpgrade() {
    if (!status) return;
    try { await openUrl(status.upgrade_url); } catch {}
  }
</script>

{#if status}
  <div
    class="tier-badge"
    title={status.classes.map((c) => `${c.label}: ${c.tier}`).join(" · ")}
  >
    <span class="badge-icon" aria-hidden="true">
      {#if showUpgrade}
        <ShieldAlert size={12} />
      {:else}
        <ShieldCheck size={12} />
      {/if}
    </span>
    {#each status.classes as c (c.class)}
      <span class={tierClass(c.tier)} aria-label={`${c.label} tier ${c.tier}`}>
        <span class="pill-label text-caption">{SHORT_LABEL[c.class] ?? c.label}</span>
        <span class="pill-value tabnum">{c.tier}</span>
      </span>
    {/each}
    {#if showUpgrade}
      <button class="upgrade-btn" onclick={handleUpgrade} title="Open ThetaData pricing page">
        <span>Upgrade</span>
        <ArrowUpRight size={11} />
      </button>
    {/if}
  </div>
{/if}

<style>
  .tier-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface-1);
    height: 26px;
    font-size: 11px;
    line-height: 1;
  }
  .badge-icon {
    display: inline-flex;
    align-items: center;
    color: var(--muted);
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 6px;
    border-radius: 6px;
    border: 1px solid transparent;
    font-variant-numeric: tabular-nums;
  }
  .pill .pill-label { color: var(--muted); }
  .pill .pill-value { font-weight: 600; letter-spacing: 0.02em; }
  .pill.unknown { background: var(--surface-2); color: var(--muted); }
  .pill.free    { background: rgba(120, 120, 120, 0.10); color: var(--text-secondary, var(--muted)); }
  .pill.value   { background: rgba(56, 132, 255, 0.10); color: rgb(56, 132, 255); border-color: rgba(56, 132, 255, 0.20); }
  .pill.standard{ background: rgba(34, 175, 109, 0.12); color: rgb(34, 175, 109); border-color: rgba(34, 175, 109, 0.22); }
  .pill.pro     { background: rgba(244, 196, 48, 0.14); color: rgb(212, 158, 0);  border-color: rgba(244, 196, 48, 0.30); }
  .upgrade-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 6px;
    border: 1px solid var(--accent, rgb(56, 132, 255));
    background: var(--accent, rgb(56, 132, 255));
    color: white;
    font-weight: 600;
    cursor: pointer;
    font-size: 11px;
    line-height: 1;
    transition: filter 120ms ease;
  }
  .upgrade-btn:hover { filter: brightness(1.08); }
  .upgrade-btn:active { filter: brightness(0.95); }
  @media (prefers-reduced-motion: reduce) {
    .upgrade-btn { transition: none; }
  }
</style>

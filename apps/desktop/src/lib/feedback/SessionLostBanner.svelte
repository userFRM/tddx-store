<script lang="ts">
  /**
   * ThetaData keeps one live session per account. When another client
   * signs in — a terminal, a script, a test suite on another machine —
   * this app's session is closed underneath it. Downloads stop, but
   * nothing is lost: every in-flight task was handed back to the queue.
   *
   * Reconnecting is left to the user on purpose. Signing back in closes
   * the *other* client's session, and doing that automatically would
   * fight whatever else is running until one of them gave up.
   */
  import { ShieldAlert, RotateCcw, Loader2 } from "lucide-svelte";
  import { app, reconnectAndResume, log } from "$lib/stores/app.svelte";

  let busy = $state(false);

  const pending = $derived(
    app.queueSnap?.counts.find(([k]) => k === "pending")?.[1] ?? 0,
  );

  async function reconnect() {
    busy = true;
    try {
      await reconnectAndResume();
    } catch (e: unknown) {
      log("error", `Reconnect failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      busy = false;
    }
  }
</script>

{#if app.sessionLost}
  <div class="banner" role="alert">
    <ShieldAlert size={18} />
    <div class="text">
      <strong>Another app signed in to your ThetaData account.</strong>
      <span>
        Only one session is live per account, so this one was closed.
        {#if pending > 0}{pending.toLocaleString()} download task{pending === 1 ? " is" : "s are"} waiting in the queue — nothing was lost.{/if}
        Reconnecting will sign the other app out.
      </span>
    </div>
    <button class="btn btn-primary btn-sm" onclick={reconnect} disabled={busy}>
      {#if busy}<Loader2 size={14} class="spin" />{:else}<RotateCcw size={14} />{/if}
      Reconnect and resume
    </button>
    <button class="btn btn-ghost btn-sm" onclick={() => (app.sessionLost = null)} disabled={busy}>
      Not now
    </button>
  </div>
{/if}

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-6);
    background: var(--warn-tint, var(--surface-2));
    border-bottom: 1px solid var(--border);
    color: var(--fg);
  }
  .banner :global(svg:first-child) { color: var(--warn); flex: none; }
  .text { display: flex; flex-direction: column; gap: 2px; flex: 1; min-width: 0; font-size: var(--text-body-sm); }
  .text span { color: var(--fg-muted); }
</style>

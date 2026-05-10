<script lang="ts">
  /**
   * Auto-popping error toasts. Each error pushed via `log("error", …)`
   * becomes a toast in the bottom-right corner with a "Copy report" /
   * "Email support" pair. Auto-dismiss after 8s; click X to dismiss
   * sooner.
   */
  import { onDestroy } from "svelte";
  import {
    AlertCircle,
    ClipboardCopy,
    Mail,
    X,
    CheckCircle2,
    Terminal,
    ArrowUpRight,
    Lock,
  } from "lucide-svelte";
  import {
    app,
    dismissToast,
    errorReport,
    errorReportMailto,
  } from "$lib/stores/app.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";

  async function upgrade() {
    const url = app.tierStatus?.upgrade_url ?? "https://thetadata.net/pricing";
    try { await openUrl(url); } catch {}
  }

  const AUTO_DISMISS_MS = 8000;
  let copiedTs = $state<number | null>(null);

  // Auto-dismiss timer per toast.
  const timers = new Map<number, ReturnType<typeof setTimeout>>();
  $effect(() => {
    for (const t of app.toasts) {
      if (!timers.has(t.ts)) {
        timers.set(
          t.ts,
          setTimeout(() => {
            dismissToast(t.ts);
            timers.delete(t.ts);
          }, AUTO_DISMISS_MS),
        );
      }
    }
  });
  onDestroy(() => {
    for (const t of timers.values()) clearTimeout(t);
    timers.clear();
  });

  async function copy(ts: number) {
    try {
      await writeText(errorReport(ts));
      copiedTs = ts;
      setTimeout(() => (copiedTs = null), 1500);
    } catch {}
  }
  async function email(ts: number) {
    try { await openUrl(errorReportMailto(ts)); } catch {}
  }
  function openConsole() {
    app.consoleOpen = true;
  }
</script>

<div class="toast-stack" aria-live="polite">
  {#each app.toasts as t (t.ts)}
    <div class="toast" class:tier-denied={t.tierDenied} role="alert">
      <span class="icon">
        {#if t.tierDenied}
          <Lock size={14} />
        {:else}
          <AlertCircle size={14} />
        {/if}
      </span>
      <div class="body">
        <div class="msg">
          {#if t.tierDenied}
            <strong>Subscription required.</strong>
            Your ThetaData tier doesn't include this dataset.
          {:else}
            {t.msg}
          {/if}
        </div>
        <div class="actions">
          {#if t.tierDenied}
            <button class="action upgrade" onclick={upgrade}>
              <ArrowUpRight size={11} /> Upgrade
            </button>
            <button class="action ghost" onclick={() => copy(t.ts_target)}>
              {#if copiedTs === t.ts_target}
                <CheckCircle2 size={11} /> Copied
              {:else}
                <ClipboardCopy size={11} /> Copy report
              {/if}
            </button>
          {:else}
            <button class="action" onclick={() => copy(t.ts_target)}>
              {#if copiedTs === t.ts_target}
                <CheckCircle2 size={11} /> Copied
              {:else}
                <ClipboardCopy size={11} /> Copy report
              {/if}
            </button>
            <button class="action" onclick={() => email(t.ts_target)}>
              <Mail size={11} /> Email support
            </button>
            <button class="action ghost" onclick={openConsole}>
              <Terminal size={11} /> Console
            </button>
          {/if}
        </div>
      </div>
      <button class="close" onclick={() => dismissToast(t.ts)} aria-label="Dismiss">
        <X size={12} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toast-stack {
    position: fixed;
    bottom: var(--sp-4);
    right: var(--sp-4);
    z-index: 110;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    width: 400px;
    max-width: calc(100vw - var(--sp-6));
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: var(--sp-2);
    align-items: flex-start;
    background: var(--surface-2);
    border: 1px solid rgba(255, 126, 126, 0.25);
    border-left: 3px solid var(--bad);
    border-radius: var(--r-md);
    padding: var(--sp-3);
    box-shadow: var(--shadow-modal);
    animation: slide-in var(--dur-base) var(--ease-standard);
  }
  @keyframes slide-in {
    from { transform: translateX(20px); opacity: 0; }
    to   { transform: translateX(0);    opacity: 1; }
  }
  .icon { color: var(--bad); padding-top: 1px; }
  .body { min-width: 0; }
  .msg {
    color: var(--fg);
    font-size: var(--text-body-sm);
    line-height: 1.4;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .actions {
    display: flex;
    gap: 6px;
    margin-top: var(--sp-2);
    flex-wrap: wrap;
  }
  .action {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px var(--sp-2);
    background: var(--surface-3);
    border: 1px solid var(--border);
    color: var(--fg-muted);
    border-radius: var(--r-sm);
    font-size: 11px;
    cursor: pointer;
  }
  .action:hover {
    background: var(--accent-tint);
    color: var(--accent-hi);
    border-color: rgba(124,140,255,0.3);
  }
  .action.ghost { background: transparent; }
  .close {
    background: transparent;
    border: 0;
    color: var(--fg-subtle);
    cursor: pointer;
    padding: 0;
    margin-left: var(--sp-1);
  }
  .close:hover { color: var(--fg); }

  /* Tier-denied variant: warm border + amber icon. The Upgrade action
     gets the accent fill so it's the obvious primary CTA. */
  .toast.tier-denied {
    border: 1px solid rgba(244, 196, 48, 0.32);
    border-left: 3px solid rgb(212, 158, 0);
  }
  .toast.tier-denied .icon { color: rgb(212, 158, 0); }
  .action.upgrade {
    background: var(--accent, rgb(56, 132, 255));
    color: white;
    border-color: var(--accent, rgb(56, 132, 255));
    font-weight: 600;
  }
  .action.upgrade:hover { filter: brightness(1.08); color: white; }
</style>

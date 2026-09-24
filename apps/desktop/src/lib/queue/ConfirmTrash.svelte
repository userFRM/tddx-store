<script lang="ts">
  /**
   * Confirmation before files go to the Trash. The count and size come
   * from the backend's own resolution of the selection, the same one
   * the delete acts on, so what is promised is what happens.
   */
  import { Trash2, Loader2 } from "lucide-svelte";
  import { api, fmtBytes, fmtNum, type LibraryTargets } from "$lib/api";

  let {
    targets,
    names,
    onconfirm,
    oncancel,
  }: {
    targets: LibraryTargets;
    /** What the user selected, as they would name it. */
    names: string[];
    onconfirm: () => Promise<void>;
    oncancel: () => void;
  } = $props();

  let total = $state<{ files: number; bytes: number } | null>(null);
  let err = $state("");
  let working = $state(false);
  let confirmBtn = $state<HTMLButtonElement | null>(null);

  $effect(() => {
    api.libraryResolve(targets).then(
      (t) => { total = t; },
      (e: unknown) => { err = e instanceof Error ? e.message : String(e); },
    );
  });
  $effect(() => { if (total && confirmBtn) confirmBtn.focus(); });

  async function confirm() {
    if (!total || total.files === 0 || working) return;
    working = true;
    try {
      await onconfirm();
    } catch (e: unknown) {
      err = e instanceof Error ? e.message : String(e);
      working = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && !working) oncancel();
  }
</script>

<div class="backdrop" role="presentation" onclick={() => !working && oncancel()}>
  <div
    class="card"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="trash-title"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={onKey}
  >
    <div class="icon"><Trash2 size={18} /></div>
    <div class="body">
      <h2 id="trash-title">
        {#if total}
          Move {fmtNum(total.files)} {total.files === 1 ? "file" : "files"} to the Trash?
        {:else}
          Move to the Trash?
        {/if}
      </h2>
      <p class="fg-muted">
        {#if total}
          {fmtBytes(total.bytes)} from
        {/if}
        {names.slice(0, 4).join(", ")}{names.length > 4 ? `, and ${names.length - 4} more` : ""}.
        You can put them back from the Trash in Finder.
      </p>
      {#if err}<p class="err">{err}</p>{/if}
      <div class="actions">
        <button class="btn btn-ghost" onclick={oncancel} disabled={working}>Cancel</button>
        <button
          class="btn trash-btn"
          bind:this={confirmBtn}
          onclick={confirm}
          disabled={!total || total.files === 0 || working}
        >
          {#if working || !total}<Loader2 size={14} class="spin" />{/if}
          Move to Trash
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed; inset: 0; z-index: 150;
    background: var(--scrim);
    backdrop-filter: blur(4px);
    display: flex; align-items: center; justify-content: center;
    padding: var(--sp-4);
  }
  .card {
    display: flex; gap: var(--sp-4);
    width: 100%; max-width: 460px;
    padding: var(--sp-5);
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-modal);
    outline: none;
  }
  .icon {
    flex: none;
    display: flex; align-items: center; justify-content: center;
    width: 36px; height: 36px;
    border-radius: 50%;
    background: var(--bad-tint);
    color: var(--bad);
  }
  h2 { margin: 0 0 6px; font-size: var(--text-heading); font-weight: var(--weight-semi); }
  p { margin: 0; font-size: var(--text-body-sm); line-height: 1.45; overflow-wrap: anywhere; }
  .err { color: var(--bad); margin-top: var(--sp-2); }
  .actions { display: flex; justify-content: flex-end; gap: var(--sp-2); margin-top: var(--sp-4); }
  .trash-btn {
    border: 1px solid var(--bad);
    color: var(--bad);
    background: var(--bad-tint);
    font-weight: var(--weight-semi);
  }
  .trash-btn:hover:not(:disabled) { background: var(--bad); color: #fff; }
</style>

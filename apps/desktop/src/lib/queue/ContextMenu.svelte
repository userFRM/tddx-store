<script lang="ts">
  /**
   * Right-click menu. Positioned at the pointer and kept inside the
   * window; closes on Escape, on a click anywhere else, and on scroll.
   */
  export type MenuItem =
    | { separator: true }
    | {
        separator?: false;
        label: string;
        shortcut?: string;
        danger?: boolean;
        disabled?: boolean;
        run: () => void;
      };

  let {
    x,
    y,
    items,
    onclose,
  }: { x: number; y: number; items: MenuItem[]; onclose: () => void } = $props();

  let el = $state<HTMLDivElement | null>(null);
  let pos = $state({ left: 0, top: 0 });

  $effect(() => {
    if (!el) return;
    const r = el.getBoundingClientRect();
    pos = {
      left: Math.min(x, window.innerWidth - r.width - 8),
      top: Math.min(y, window.innerHeight - r.height - 8),
    };
    el.focus();
  });

  function pick(item: MenuItem) {
    if (item.separator || item.disabled) return;
    onclose();
    item.run();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window
  onmousedown={(e) => { if (el && !el.contains(e.target as Node)) onclose(); }}
  onblur={onclose}
/>
<svelte:document onscrollcapture={onclose} />

<div
  class="menu"
  role="menu"
  tabindex="-1"
  bind:this={el}
  style="left: {pos.left}px; top: {pos.top}px"
  onkeydown={onKey}
>
  {#each items as item, i (i)}
    {#if item.separator}
      <div class="sep" role="separator"></div>
    {:else}
      <button
        type="button"
        role="menuitem"
        class="item"
        class:danger={item.danger}
        disabled={item.disabled}
        onclick={() => pick(item)}
      >
        <span>{item.label}</span>
        {#if item.shortcut}<kbd>{item.shortcut}</kbd>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 200;
    min-width: 220px;
    padding: 4px;
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-modal);
    outline: none;
  }
  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
    width: 100%;
    padding: 5px 10px;
    border: none;
    border-radius: var(--r-sm);
    background: none;
    color: var(--fg);
    font: inherit;
    font-size: var(--text-body-sm);
    text-align: left;
    cursor: pointer;
  }
  .item:hover:not(:disabled), .item:focus-visible {
    background: var(--accent-fill);
    color: #fff;
    outline: none;
  }
  .item:hover:not(:disabled) kbd, .item:focus-visible kbd { color: rgba(255, 255, 255, 0.8); }
  .item:disabled { color: var(--fg-subtle); cursor: default; }
  .item.danger { color: var(--bad); }
  .item.danger:hover:not(:disabled) { background: var(--bad); color: #fff; }
  kbd {
    font-family: var(--font-ui);
    font-size: var(--text-caption);
    color: var(--fg-subtle);
  }
  .sep { height: 1px; margin: 4px 6px; background: var(--border); }
</style>

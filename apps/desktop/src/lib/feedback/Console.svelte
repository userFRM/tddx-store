<script lang="ts">
  /**
   * Slide-in activity console. Hosts the in-memory log of every notable
   * action (login, queue ops, errors, retries) and offers one-click
   * "Copy report" for ThetaData support.
   */
  import {
    X,
    Trash2,
    ClipboardCopy,
    AlertCircle,
    AlertTriangle,
    Info,
    CheckCircle2,
    Mail,
    Send,
  } from "lucide-svelte";
  import {
    app,
    clearLog,
    activityReport,
    errorReport,
    errorReportMailto,
    type LogLevel,
  } from "$lib/stores/app.svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { openUrl } from "@tauri-apps/plugin-opener";

  let copied = $state(false);
  let perRowCopied = $state<number | null>(null);
  let activeFilter = $state<LogLevel | "all">("all");

  async function copyErrorReport(ts: number) {
    try {
      await writeText(errorReport(ts));
      perRowCopied = ts;
      setTimeout(() => (perRowCopied = null), 1500);
    } catch {
      perRowCopied = null;
    }
  }
  async function emailErrorReport(ts: number) {
    try { await openUrl(errorReportMailto(ts)); } catch {}
  }

  function close() { app.consoleOpen = false; }

  async function copyReport() {
    try {
      await writeText(activityReport());
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      copied = false;
    }
  }

  const filtered = $derived(
    activeFilter === "all"
      ? app.activity
      : app.activity.filter((e) => e.level === activeFilter),
  );

  function levelColor(l: LogLevel): string {
    switch (l) {
      case "error": return "var(--bad)";
      case "warn":  return "var(--warn)";
      case "info":  return "var(--accent-hi)";
      case "debug": return "var(--fg-subtle)";
    }
  }

  function fmtTime(ts: number) {
    return new Date(ts).toLocaleTimeString(undefined, { hour12: false });
  }

  const counts = $derived(
    app.activity.reduce(
      (acc, e) => {
        acc[e.level] = (acc[e.level] ?? 0) + 1;
        return acc;
      },
      {} as Record<string, number>,
    ),
  );
</script>

{#if app.consoleOpen}
  <aside class="console" role="complementary" aria-label="Activity console">
    <header class="console-header">
      <div class="console-title-block">
        <span class="text-caption">Console</span>
        <h2 class="console-title">Activity log</h2>
      </div>
      <div class="console-actions">
        <button class="btn btn-ghost" onclick={copyReport}>
          <ClipboardCopy size={14} />
          {copied ? "Copied" : "Copy report"}
        </button>
        <button class="btn btn-ghost" onclick={clearLog}>
          <Trash2 size={14} /> Clear
        </button>
        <button class="btn-icon" onclick={close} aria-label="Close console">
          <X size={14} />
        </button>
      </div>
    </header>

    <div class="filter-row">
      {#each ["all", "info", "warn", "error", "debug"] as f}
        {@const count = f === "all" ? app.activity.length : (counts[f as LogLevel] ?? 0)}
        <button
          class="filter-pill"
          class:active={activeFilter === f}
          onclick={() => (activeFilter = f as typeof activeFilter)}
        >
          {f}
          <span class="filter-count tabnum">{count}</span>
        </button>
      {/each}
    </div>

    <div class="console-scroll" role="log">
      {#if filtered.length === 0}
        <div class="empty">
          <span class="text-body-sm fg-muted">No activity yet.</span>
        </div>
      {:else}
        <ul class="log-list">
          {#each filtered as entry, i (entry.ts + ":" + i)}
            <li class="log-row" data-level={entry.level}>
              <span class="log-icon" style:color={levelColor(entry.level)}>
                {#if entry.level === "error"}
                  <AlertCircle size={12} />
                {:else if entry.level === "warn"}
                  <AlertTriangle size={12} />
                {:else if entry.level === "info"}
                  <CheckCircle2 size={12} />
                {:else}
                  <Info size={12} />
                {/if}
              </span>
              <span class="log-time text-mono">{fmtTime(entry.ts)}</span>
              <span class="log-msg">{entry.msg}</span>
              {#if entry.level === "error"}
                <span class="log-actions">
                  <button class="row-btn" title="Copy support report"
                          onclick={() => copyErrorReport(entry.ts)}>
                    {#if perRowCopied === entry.ts}
                      <CheckCircle2 size={11} />
                      Copied
                    {:else}
                      <ClipboardCopy size={11} />
                      Report
                    {/if}
                  </button>
                  <button class="row-btn" title="Email ThetaData support"
                          onclick={() => emailErrorReport(entry.ts)}>
                    <Mail size={11} />
                    Email
                  </button>
                </span>
              {/if}
              {#if entry.context}
                <span class="log-ctx text-mono">
                  {Object.entries(entry.context)
                    .map(([k, v]) => `${k}=${v}`)
                    .join(" ")}
                </span>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <footer class="console-footer">
      <span class="hint fg-muted">
        {app.activity.length} entries · keeps last 1,000.
      </span>
      {#if Object.keys(counts).includes("error")}
        <a
          class="support-link"
          href="mailto:support@thetadata.us?subject=ThetaDataDx%20Downloader%20issue&body={encodeURIComponent('Please paste the activity report here.\n\n')}"
        >
          Email ThetaData support →
        </a>
      {/if}
    </footer>
  </aside>
{/if}

<style>
  .console {
    position: fixed;
    top: 48px;
    right: 0;
    bottom: 0;
    width: 480px;
    max-width: 100vw;
    background: var(--surface-1);
    border-left: 1px solid var(--border-strong);
    box-shadow: var(--shadow-modal);
    display: flex;
    flex-direction: column;
    z-index: 90;
    animation: slide-in var(--dur-base) var(--ease-standard);
  }
  @keyframes slide-in {
    from { transform: translateX(20px); opacity: 0; }
    to   { transform: translateX(0);    opacity: 1; }
  }

  .console-header {
    padding: var(--sp-4) var(--sp-4) var(--sp-3);
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: var(--sp-3);
    border-bottom: 1px solid var(--border);
  }
  .console-title-block { display: flex; flex-direction: column; gap: 2px; }
  .console-title {
    font-family: var(--font-display);
    font-size: var(--text-display-lg);
    font-weight: var(--weight-semi);
    margin: 0;
  }
  .console-actions { display: flex; gap: var(--sp-1); align-items: center; }

  .filter-row {
    display: flex;
    gap: var(--sp-1);
    padding: var(--sp-2) var(--sp-4);
    border-bottom: 1px solid var(--border);
    overflow-x: auto;
  }
  .filter-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px var(--sp-2);
    background: transparent;
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    color: var(--fg-muted);
    font-size: var(--text-caption);
    font-weight: var(--weight-medium);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    cursor: pointer;
  }
  .filter-pill:hover { background: var(--surface-2); color: var(--fg); }
  .filter-pill.active {
    background: var(--accent-tint);
    color: var(--accent-hi);
    border-color: rgba(124, 140, 255, 0.4);
  }
  .filter-count {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--fg-subtle);
  }
  .filter-pill.active .filter-count { color: var(--accent-hi); }

  .console-scroll {
    flex: 1;
    overflow-y: auto;
    padding: var(--sp-2) 0;
  }
  .log-list { list-style: none; padding: 0; margin: 0; }
  .log-row {
    display: grid;
    grid-template-columns: 16px 76px 1fr;
    gap: var(--sp-2);
    align-items: baseline;
    padding: 6px var(--sp-4);
    border-left: 2px solid transparent;
    font-size: var(--text-body-sm);
  }
  .log-row[data-level="error"] { border-left-color: var(--bad); }
  .log-row[data-level="warn"]  { border-left-color: var(--warn); }
  .log-row[data-level="info"]  { border-left-color: var(--accent); }
  .log-row:hover { background: var(--surface-2); }

  .log-icon { display: inline-flex; align-items: center; }
  .log-time {
    color: var(--fg-subtle);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
  .log-msg { color: var(--fg); }
  .log-ctx {
    grid-column: 3 / 4;
    color: var(--fg-subtle);
    font-size: 11px;
    margin-top: 2px;
  }

  .log-actions {
    grid-column: 3 / 4;
    margin-top: 4px;
    display: inline-flex;
    gap: 6px;
  }
  .row-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--fg-muted);
    border-radius: var(--r-sm);
    font-size: 11px;
    cursor: pointer;
  }
  .row-btn:hover {
    background: var(--accent-tint);
    color: var(--accent-hi);
    border-color: rgba(124,140,255,0.3);
  }

  .empty {
    padding: var(--sp-8);
    text-align: center;
  }

  .console-footer {
    padding: var(--sp-3) var(--sp-4);
    border-top: 1px solid var(--border);
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--sp-3);
  }
  .hint { font-size: var(--text-body-sm); }
  .support-link {
    color: var(--accent);
    font-size: var(--text-body-sm);
    text-decoration: none;
  }
  .support-link:hover { color: var(--accent-hi); }
</style>

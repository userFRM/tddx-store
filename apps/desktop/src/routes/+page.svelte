<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    Home,
    Compass,
    Library,
    ListChecks,
    CalendarClock,
    HeartPulse,
    Settings as SettingsIcon,
    Search,
    Activity,
    AlertCircle,
    CheckCircle2,
    Terminal,
    Sun,
    Moon,
    Monitor,
  } from "lucide-svelte";

  import HomeView from "$lib/home/HomeView.svelte";
  import BrowseView from "$lib/catalogue/BrowseView.svelte";
  import LibraryView from "$lib/queue/LibraryView.svelte";
  import QueueView from "$lib/queue/QueueView.svelte";
  import DetailView from "$lib/catalogue/DetailView.svelte";
  import SchedulesView from "$lib/schedules/SchedulesView.svelte";
  import HealthView from "$lib/health/HealthView.svelte";
  import Settings from "$lib/settings/Settings.svelte";
  import ActiveDownloadsPane from "$lib/queue/ActiveDownloadsPane.svelte";
  import AddModal from "$lib/composer/AddModal.svelte";
  import LoginGate from "$lib/settings/LoginGate.svelte";
  import Console from "$lib/feedback/Console.svelte";
  import ErrorToasts from "$lib/feedback/ErrorToasts.svelte";
  import CommandPalette from "$lib/feedback/CommandPalette.svelte";
  import DataViewer from "$lib/queue/DataViewer.svelte";
  import { loadSavedSearches } from "$lib/persistence/savedSearches";
  import EndpointRunner from "$lib/runners/EndpointRunner.svelte";
  import FlatfileRunner from "$lib/runners/FlatfileRunner.svelte";
  import IndexPresetRunner from "$lib/runners/IndexPresetRunner.svelte";
  import TierBadge from "$lib/tier/TierBadge.svelte";
  import { installMarkdownLinkInterceptor } from "$lib/util/md";
  import { getVersion } from "@tauri-apps/api/app";

  // Resolved at runtime from the Tauri `app` API rather than imported
  // from `package.json` so the value comes from the bundled binary's
  // Cargo manifest — single source of truth, survives version bumps
  // that touch `tauri.conf.json` without re-running the FE build.
  let APP_VERSION = $state("");
  $effect(() => {
    getVersion().then((v) => (APP_VERSION = v)).catch(() => {});
  });

  import {
    app,
    navigate,
    closeComposer,
    stopQueuePoll,
    startProgressListener,
    stopProgressListener,
    loadSettings,
    loadCatalogue,
    cycleTheme,
    type View,
  } from "$lib/stores/app.svelte";

  // ── On mount: load settings + saved searches + yaml catalogue;
  //     LoginGate auto-connects. The catalogue comes from the
  //     vendored yaml + auto-fetched runtime override — does NOT
  //     need a connection so the dataset store is browseable
  //     even before sign-in.
  onMount(async () => {
    installMarkdownLinkInterceptor();
    await loadSettings();
    void loadSavedSearches();
    void loadCatalogue();
    // Subscribe to backend progress events so the UI is push-updated
    // (Tier 2 "real" progress); SQLite polling stays as the safety net.
    startProgressListener();
  });
  onDestroy(() => {
    stopQueuePoll();
    stopProgressListener();
  });
  onDestroy(stopQueuePoll);

  function toggleConsole() {
    app.consoleOpen = !app.consoleOpen;
  }

  function handleConnIndicatorClick() {
    if (app.connState !== "connected") navigate("settings");
  }

  // ── Cmd/Ctrl-K global focus on search (placeholder for now) ──
  let searchEl = $state<HTMLInputElement | undefined>(undefined);
  let searchQuery = $state("");
  function onKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      searchEl?.focus();
    }
    if (e.key === "Escape" && app.composer.open) {
      closeComposer();
    }
  }

  const navItems: { id: View; label: string; icon: typeof Compass }[] = [
    { id: "home",      label: "Home",      icon: Home },
    { id: "browse",    label: "Browse",    icon: Compass },
    { id: "library",   label: "Library",   icon: Library },
    { id: "queue",     label: "Queue",     icon: ListChecks },
    { id: "schedules", label: "Schedules", icon: CalendarClock },
    { id: "health",    label: "Health",    icon: HeartPulse },
    { id: "settings",  label: "Settings",  icon: SettingsIcon },
  ];

  const queuedCount = $derived(
    (app.queueSnap?.counts.find(([s]) => s === "running")?.[1] ?? 0)
  );
</script>

<svelte:window onkeydown={onKey} />

<div class="app">
  <!-- ── Top bar ──────────────────────────────────────────── -->
  <header class="topbar" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <img
        class="brand-logo"
        src="/thetadata-logo.svg"
        alt="ThetaData"
        draggable="false"
      />
      <span class="brand-name">Store</span>
      <span class="brand-tag text-caption">v{APP_VERSION}</span>
    </div>

    <div class="search-wrap">
      <Search size={14} class="search-icon" />
      <input
        bind:this={searchEl}
        bind:value={searchQuery}
        class="search-input"
        type="search"
        placeholder="Search datasets, symbols, endpoints…  (⌘K)"
      />
    </div>

    <div class="topbar-right">
    <TierBadge />
    <button
      class="btn-icon"
      onclick={cycleTheme}
      aria-label="Toggle theme"
      title={app.themePref === "system"
        ? `System theme (now ${app.themeResolved})`
        : `${app.themePref.charAt(0).toUpperCase()}${app.themePref.slice(1)} theme`}
    >
      {#if app.themePref === "system"}
        <Monitor size={14} />
      {:else if app.themePref === "light"}
        <Sun size={14} />
      {:else}
        <Moon size={14} />
      {/if}
    </button>
    <button
      class="btn-icon console-toggle"
      class:has-errors={app.activity.some((e) => e.level === "error")}
      onclick={toggleConsole}
      aria-label="Toggle console"
      title="Activity console"
    >
      <Terminal size={14} />
    </button>
    <button
      class="conn-indicator"
      class:connected={app.connState === "connected"}
      class:error={app.connState === "error"}
      onclick={handleConnIndicatorClick}
      title={app.connMsg}
    >
      {#if app.connState === "connected"}
        <CheckCircle2 size={12} />
      {:else if app.connState === "error"}
        <AlertCircle size={12} />
      {:else}
        <Activity size={12} />
      {/if}
      <span class="conn-label text-caption">
        {app.connState === "connected"
          ? "Connected"
          : app.connState === "connecting"
          ? "Connecting…"
          : app.connState === "error"
          ? "Disconnected"
          : "Idle"}
      </span>
    </button>
    </div>
  </header>

  <!-- ── Body: rail · content · downloads pane ─────────────── -->
  <div class="body">
    <nav class="rail" class:collapsed={app.railCollapsed}>
      {#each navItems as item}
        {@const Icon = item.icon}
        <button
          class="rail-item"
          class:active={app.currentView === item.id}
          onclick={() => navigate(item.id)}
        >
          <Icon size={18} />
          {#if !app.railCollapsed}<span class="rail-label">{item.label}</span>{/if}
          {#if item.id === "queue" && queuedCount > 0}
            <span class="rail-badge tabnum">{queuedCount}</span>
          {/if}
        </button>
      {/each}
    </nav>

    <main class="main">
      {#if app.currentView === "detail" && app.detailDataset}
        <DetailView />
      {:else if app.currentView === "home"}
        <HomeView />
      {:else if app.currentView === "browse"}
        <BrowseView />
      {:else if app.currentView === "library"}
        <LibraryView />
      {:else if app.currentView === "queue"}
        <QueueView />
      {:else if app.currentView === "schedules"}
        <SchedulesView />
      {:else if app.currentView === "health"}
        <HealthView />
      {:else if app.currentView === "settings"}
        <Settings />
      {/if}
    </main>

    <ActiveDownloadsPane />
  </div>
</div>

<!-- Composer popover lives globally so it can anchor anywhere -->
<AddModal />

<!-- Generic dispatcher modal (any registered endpoint) -->
<EndpointRunner />

<!-- Flatfile downloader -->
<FlatfileRunner />

<!-- Index ecosystem bulk-queue -->
<IndexPresetRunner />

<!-- Activity console (slide-in) -->
<Console />

<!-- Error toasts -->
<ErrorToasts />

<!-- Login gate hides until connState === "connected" -->
<LoginGate />

<!-- Cmd/Ctrl-K palette — global accelerator -->
<CommandPalette />

<!-- Parquet data viewer modal (used by Library "Sample" buttons) -->
<DataViewer
  bind:open={app.viewer.open}
  path={app.viewer.path}
  title={app.viewer.title}
/>

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--fg);
  }

  /* ── Top bar ─────────────────────────────────────────────── */
  .topbar {
    display: grid;
    grid-template-columns: 240px 1fr auto;
    align-items: center;
    gap: var(--sp-4);
    padding: 0 var(--sp-4);
    height: 48px;
    background: var(--surface-1);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }
  .brand-logo {
    height: 22px;
    width: auto;
    display: block;
    -webkit-user-select: none;
    user-select: none;
  }
  .brand-name {
    font-size: var(--text-heading);
    font-weight: var(--weight-semi);
    letter-spacing: -0.01em;
    color: var(--fg-muted);
    margin-left: 2px;
  }
  .brand-tag {
    color: var(--fg-subtle);
    margin-left: var(--sp-1);
  }

  .search-wrap {
    position: relative;
    max-width: 540px;
    margin: 0 auto;
    width: 100%;
  }
  .search-wrap :global(.search-icon) {
    position: absolute;
    left: var(--sp-3);
    top: 50%;
    transform: translateY(-50%);
    color: var(--fg-subtle);
    pointer-events: none;
  }
  .search-input {
    width: 100%;
    height: 30px;
    padding: 0 var(--sp-3) 0 32px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    color: var(--fg);
    font-size: var(--text-body-sm);
    outline: none;
    transition: border-color var(--dur-fast) var(--ease-standard);
  }
  .search-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-tint);
  }
  .search-input::placeholder { color: var(--fg-subtle); }

  .topbar-right {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
  }
  .console-toggle {
    position: relative;
    width: 28px;
    height: 28px;
    border-radius: var(--r-sm);
  }
  .console-toggle.has-errors::after {
    content: "";
    position: absolute;
    top: 4px;
    right: 4px;
    width: 6px;
    height: 6px;
    background: var(--bad);
    border-radius: 50%;
  }

  .conn-indicator {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    padding: 4px var(--sp-3);
    height: 24px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    color: var(--fg-muted);
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-standard),
                color var(--dur-fast) var(--ease-standard),
                border-color var(--dur-fast) var(--ease-standard);
  }
  .conn-indicator:hover {
    background: var(--surface-3);
    color: var(--fg);
  }
  .conn-indicator.connected {
    color: var(--good);
    border-color: rgba(93, 212, 160, 0.3);
  }
  .conn-indicator.error {
    color: var(--bad);
    border-color: rgba(255, 126, 126, 0.3);
  }
  .conn-label {
    color: inherit;
  }

  /* ── Body grid ──────────────────────────────────────────── */
  .body {
    flex: 1;
    display: grid;
    grid-template-columns: auto 1fr auto;
    min-height: 0;
  }

  /* ── Left rail ──────────────────────────────────────────── */
  .rail {
    width: var(--rail-w);
    flex-shrink: 0;
    background: var(--surface-1);
    border-right: 1px solid var(--border);
    padding: var(--sp-3) var(--sp-2);
    display: flex;
    flex-direction: column;
    gap: 2px;
    transition: width var(--dur-base) var(--ease-standard);
  }
  .rail.collapsed { width: var(--rail-w-sm); }

  .rail-item {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 0 var(--sp-3);
    height: 36px;
    width: 100%;
    background: transparent;
    color: var(--fg-muted);
    border: none;
    border-radius: var(--r-sm);
    font-size: var(--text-body-sm);
    font-weight: var(--weight-medium);
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-standard),
                color var(--dur-fast) var(--ease-standard);
    position: relative;
  }
  .rail-item:hover {
    background: var(--surface-2);
    color: var(--fg);
  }
  .rail-item.active {
    background: var(--accent-tint);
    color: var(--accent-hi);
  }
  .rail-label {
    flex: 1;
    text-align: left;
  }
  .rail-badge {
    margin-left: auto;
    font-size: var(--text-caption);
    background: var(--accent);
    color: #fff;
    padding: 1px 6px;
    border-radius: var(--r-pill);
    font-weight: var(--weight-semi);
  }

  /* ── Main content ───────────────────────────────────────── */
  .main {
    overflow: hidden;
    display: flex;
    flex-direction: column;
    min-width: 0;
    background: var(--bg);
  }
</style>

// Central application state using Svelte 5 runes.
//
// Svelte 5 forbids re-assigning to module-level imported `let $state(...)`
// bindings from another module. The workaround is one exported `$state`
// object whose *fields* are reactive and freely mutable across modules.
//
// Usage from a component:
//   import { app, openComposer } from "$lib/stores/app.svelte";
//   app.currentView = "browse";
//   openComposer(dataset);

import {
  api,
  governingTierForKind,
  minTierForKind,
  resolveKindToEndpoint,
  tierMeets,
  TIER_RANK,
  DEFAULT_PREFERENCES,
  type CatalogueEntry,
  type Coverage,
  type EndpointInfo,
  type QueueSnapshot,
  type Settings,
  type TierName,
  type TierStatus,
  type TierVerdict,
} from "$lib/api";

// ── Types ────────────────────────────────────────────────────
export type View =
  | "home"
  | "browse"
  | "library"
  | "queue"
  | "schedules"
  | "health"
  | "settings"
  | "detail";
export type ConnState = "idle" | "connecting" | "connected" | "error";
export type ThemePref = "system" | "light" | "dark";
export type ThemeResolved = "light" | "dark";
export type AssetClass = "stock" | "option" | "index" | "rate";
export type Cadence =
  | "trade"
  | "quote"
  | "trade_quote"
  | "eod"
  | "snapshot"
  | "greeks"
  | "oi";

export interface DatasetMeta {
  id: string;
  title: string;
  subtitle: string;
  assetClass: AssetClass;
  cadence: Cadence;
  specLine: string;
  featured: boolean;
  tags: string[];
}

export type StrikeUnit = "auto" | "dollars" | "thousands";
export interface FieldRename { from: string; to: string }

export interface ComposerState {
  open: boolean;
  anchorDataset: DatasetMeta | null;
  symbol: string;
  start: string;
  end: string;
  format: "parquet" | "csv" | "jsonl" | "json";
  interval: string;
  expiration: string;
  strike: string;
  right: "both" | "C" | "P";
  status: "idle" | "queuing" | "done" | "error";
  msg: string;
  // Advanced (collapsed by default)
  advancedOpen: boolean;
  strikeUnit: StrikeUnit;
  renames: FieldRename[];
  drops: string[];
}

// ── Activity log entry ──────────────────────────────────────
export type LogLevel = "info" | "warn" | "error" | "debug";
export interface LogEntry {
  ts: number;          // Unix ms
  level: LogLevel;
  msg: string;
  context?: Record<string, string | number | boolean | null>;
}

// Transient on-screen toasts — purely UI surface for errors.
export interface Toast {
  ts: number;          // toast creation
  msg: string;
  ts_target: number;   // matching activity entry ts (for "Report")
  /** When the error came from a tier-insufficient gRPC PermissionDenied,
   *  ErrorToasts swaps the support actions for an Upgrade CTA. */
  tierDenied?: boolean;
}

// ── Symbol cache (warmed on connect) ────────────────────────
export interface SymbolCache {
  loadedAt: number | null;
  loading: boolean;
  stockSymbols: string[];
  optionRoots: string[];
}

// ── Single global app state object ───────────────────────────
interface AppState {
  // Navigation
  currentView: View;
  detailDataset: DatasetMeta | null;
  railCollapsed: boolean;
  cmdkOpen: boolean;
  consoleOpen: boolean;
  // Queue / connection
  queueSnap: QueueSnapshot | null;
  queuePollActive: boolean;
  connState: ConnState;
  connMsg: string;
  // Composer
  composer: ComposerState;
  // Settings
  settings: Settings;
  // Activity log (capped ring buffer)
  activity: LogEntry[];
  // Error toasts (auto-dismissed after a few seconds by the UI)
  toasts: Toast[];
  // Warm symbol caches
  symbols: SymbolCache;
  // Theme
  themePref: ThemePref;
  themeResolved: ThemeResolved;
  // Saved searches (localStorage-backed; loaded on mount)
  savedSearches: import("$lib/persistence/savedSearches").SavedSearch[];
  // Cmd-K palette
  cmdkOpen2: boolean;
  // Data viewer
  viewer: { open: boolean; path: string; title: string };
  // Live in-flight task ids (pushed by the worker event stream)
  runningTaskIds: string[];
  // Endpoint runner (one-shot dispatcher modal for any registered endpoint)
  endpointRunnerOpen: boolean;
  endpointRunner: EndpointRunnerState | null;
  // Flatfiles modal
  flatfileRunnerOpen: boolean;
  // Index preset modal
  presetOpen: boolean;
  presetSelected: IndexPreset | null;
  // Subscription-tier gating
  tierStatus: TierStatus | null;
  tierVerdicts: TierVerdict[];
  // YAML-driven endpoint catalogue (loaded on connect)
  catalogue: CatalogueEntry[];
  catalogueLoading: boolean;
  /** A dataset + symbol another view wants Browse to open on. Browse
   *  consumes it once and clears it. */
  browseIntent: { kind: string; symbol: string } | null;
  /** What is on disk. Shared rather than per-view so Home and Library
   *  cannot disagree, and so a finished download can invalidate it in
   *  one place. */
  coverage: Coverage[];
  coverageLoading: boolean;
}

export interface EndpointRunnerState {
  endpoint: EndpointInfo;
  args: Record<string, string>;
  format: "parquet" | "csv" | "jsonl" | "json";
  busy: boolean;
  msg: string;
  result?: unknown;
}

export type IndexPreset = {
  id: string;
  name: string;
  description: string;
  symbols: string[];   // populated from indexkit on demand
};

export const app = $state<AppState>({
  currentView: "home",
  detailDataset: null,
  railCollapsed: false,
  cmdkOpen: false,
  consoleOpen: false,
  queueSnap: null,
  queuePollActive: false,
  connState: "idle",
  connMsg: "",
  composer: {
    open: false,
    anchorDataset: null,
    symbol: "",
    start: "",
    end: "",
    format: "parquet",
    interval: "tick",
    expiration: "*",
    strike: "*",
    right: "both",
    status: "idle",
    msg: "",
    advancedOpen: false,
    strikeUnit: "auto",
    renames: [],
    drops: [],
  },
  settings: {
    db_path: "",
    output_dir: "",
    creds_path: "",
    preferences: { ...DEFAULT_PREFERENCES, watchlists: [] },
  },
  activity: [],
  toasts: [],
  symbols: {
    loadedAt: null,
    loading: false,
    stockSymbols: [],
    optionRoots: [],
  },
  themePref: "system",
  themeResolved: "light",
  savedSearches: [],
  cmdkOpen2: false,
  viewer: { open: false, path: "", title: "" },
  runningTaskIds: [],
  endpointRunnerOpen: false,
  endpointRunner: null,
  flatfileRunnerOpen: false,
  presetOpen: false,
  presetSelected: null,
  tierStatus: null,
  tierVerdicts: [],
  catalogue: [],
  catalogueLoading: false,
  browseIntent: null,
  coverage: [],
  coverageLoading: false,
});

// ── Theme ─────────────────────────────────────────────────────
//
// Persisted via tauri-plugin-store (kv.ts). The plugin writes to a
// single JSON file in the OS-correct app data dir resolved by
// `app.path().app_data_dir()` server-side, so the theme survives a
// reinstall under the same bundle identifier on every platform.
import { kvGet, kvRemove, kvSet } from "$lib/persistence/kv";
import { notify } from "$lib/persistence/notifications";

const THEME_KEY = "tdds.theme";

function systemPreferred(): ThemeResolved {
  // Light is the fallback: the ThetaData identity is a light one, and
  // an environment that cannot report a preference should land on the
  // brand surface rather than the app's own dark variant.
  if (typeof window === "undefined" || !window.matchMedia) return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function applyTheme(pref: ThemePref) {
  const resolved: ThemeResolved = pref === "system" ? systemPreferred() : pref;
  app.themePref = pref;
  app.themeResolved = resolved;
  if (typeof document !== "undefined") {
    document.documentElement.setAttribute("data-theme", resolved);
  }
  // Fire-and-forget; the in-memory mirror is the source of truth for the
  // current session, and persistence survives the next launch.
  if (pref === "system") void kvRemove(THEME_KEY);
  else void kvSet(THEME_KEY, pref);
}

export function setTheme(pref: ThemePref) {
  applyTheme(pref);
}

/** Cycle through system → light → dark → system. */
export function cycleTheme() {
  const next: ThemePref =
    app.themePref === "system" ? "light"
    : app.themePref === "light"  ? "dark"
    :                              "system";
  setTheme(next);
}

/** Read stored choice (or default to system) and apply on launch.
 *  Hooks the system colour-scheme media query so unsaved users follow
 *  OS theme changes live. Call once from +layout.svelte. */
export async function initTheme() {
  if (typeof window === "undefined") return;
  const saved = (await kvGet<ThemePref>(THEME_KEY)) ?? "system";
  applyTheme(saved);

  // Listen for OS preference changes when user is in "system" mode.
  if (window.matchMedia) {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      if (app.themePref === "system") applyTheme("system");
    };
    mq.addEventListener?.("change", onChange);
  }
}

// ── Activity log helpers ─────────────────────────────────────
const MAX_LOG_ENTRIES = 1000;

export function log(
  level: LogLevel,
  msg: string,
  context?: LogEntry["context"],
) {
  app.activity.unshift({ ts: Date.now(), level, msg, context });
  if (app.activity.length > MAX_LOG_ENTRIES) {
    app.activity.length = MAX_LOG_ENTRIES;
  }
  // Mirror to devtools console for live diagnostics.
  const fn =
    level === "error" ? console.error : level === "warn" ? console.warn : console.log;
  if (context) fn(`[${level}] ${msg}`, context);
  else fn(`[${level}] ${msg}`);
  // Push a non-blocking toast on errors so the user can act immediately
  // even when the activity console is closed.
  if (level === "error") {
    app.toasts.unshift({
      ts: Date.now(),
      msg,
      ts_target: app.activity[0].ts,
      tierDenied: detectTierDenied(msg, context),
    });
    if (app.toasts.length > 4) app.toasts.length = 4;
  }
}

/** Heuristic: does this error message indicate a tier-insufficient
 *  PermissionDenied from the gRPC server? Backend wraps it as
 *  `Error::Theta(Error::Grpc { kind: PermissionDenied, .. })`; that
 *  Display impl carries "PermissionDenied" / "permission denied" /
 *  "tier insufficient". Match conservatively. */
function detectTierDenied(msg: string, context?: LogEntry["context"]): boolean {
  if (context && (context as Record<string, unknown>).tier_denied === true) return true;
  const m = msg.toLowerCase();
  return (
    m.includes("permissiondenied") ||
    m.includes("permission denied") ||
    m.includes("tier insufficient") ||
    m.includes("not entitled") ||
    m.includes("not authorized for this subscription")
  );
}

export function clearLog() {
  app.activity = [];
}

export function dismissToast(ts: number) {
  app.toasts = app.toasts.filter((t) => t.ts !== ts);
}

/** Build a focused report around a specific error entry: ±10 min of
 *  surrounding activity, app state, and headers ThetaData support
 *  needs to triage. Returned as plain text — paste into email body. */
export function errorReport(targetTs: number): string {
  const before = 10 * 60 * 1000;
  const after = 60 * 1000;
  const window = app.activity.filter(
    (e) => e.ts >= targetTs - before && e.ts <= targetTs + after,
  );
  const target = app.activity.find((e) => e.ts === targetTs);
  const header = [
    "TdDx Store — error report",
    `Generated: ${new Date().toISOString()}`,
    `Error timestamp: ${target ? new Date(target.ts).toISOString() : "?"}`,
    `Error message: ${target?.msg ?? "?"}`,
    target?.context ? `Error context: ${JSON.stringify(target.context)}` : "",
    "",
    `Connection: ${app.connState}${app.connState === "error" ? " — " + app.connMsg : ""}`,
    `Queue counts: ${app.queueSnap?.counts.map(([s, n]) => `${s}=${n}`).join(" ") ?? "n/a"}`,
    `Files on disk: ${app.queueSnap?.files_on_disk ?? 0} (${app.queueSnap?.bytes_on_disk ?? 0} bytes)`,
    `Settings: output=${app.settings.output_dir}`,
    `User-agent: ${typeof navigator !== "undefined" ? navigator.userAgent : "n/a"}`,
    `Theme: ${app.themePref} (resolved=${app.themeResolved})`,
    "",
    `—— activity (±10 min around the error) ——`,
  ].filter(Boolean).join("\n");
  const lines = window
    .slice()
    .reverse()
    .map((e) => {
      const t = new Date(e.ts).toISOString();
      const ctx = e.context ? " " + JSON.stringify(e.context) : "";
      return `${t} [${e.level.toUpperCase().padEnd(5)}] ${e.msg}${ctx}`;
    });
  return [header, ...lines].join("\n");
}

/** mailto: URL pre-populated with `errorReport`. */
export function errorReportMailto(targetTs: number): string {
  const target = app.activity.find((e) => e.ts === targetTs);
  const subject = `TdDx Store issue: ${target?.msg.slice(0, 80) ?? "error report"}`;
  const body = errorReport(targetTs);
  const params = new URLSearchParams({ subject, body });
  return `mailto:support@thetadata.us?${params.toString()}`;
}

/** Export the activity log as a plain-text report for ThetaData support. */
export function activityReport(): string {
  const header = [
    "TdDx Store activity report",
    `Generated: ${new Date().toISOString()}`,
    `Connection: ${app.connState}${app.connState === "error" ? " — " + app.connMsg : ""}`,
    `Queue: ${app.queueSnap?.counts.map(([s, n]) => `${s}=${n}`).join(" ") ?? "n/a"}`,
    `Files on disk: ${app.queueSnap?.files_on_disk ?? 0} (${app.queueSnap?.bytes_on_disk ?? 0} bytes)`,
    `Settings: output=${app.settings.output_dir}`,
    "",
    "—— activity ——",
  ].join("\n");
  const lines = app.activity
    .slice() // avoid mutating
    .reverse()
    .map((e) => {
      const t = new Date(e.ts).toISOString();
      const ctx = e.context ? " " + JSON.stringify(e.context) : "";
      return `${t} [${e.level.toUpperCase().padEnd(5)}] ${e.msg}${ctx}`;
    });
  return [header, ...lines].join("\n");
}

// ── Live progress events from the worker pool ────────────────
//
// The Tauri backend emits `tdds:progress` events as workers go
// through Started → Done/Empty/Failed transitions. We listen once on
// app start and update the runtime task counter so the UI reacts in
// real time instead of waiting for the 1.5 s SQLite poll.

import type { UnlistenFn } from "@tauri-apps/api/event";

export type WorkerEvent =
  | { type: "started"; task_id: string }
  | { type: "done"; task_id: string; rows: number; bytes: number; millis: number }
  | { type: "empty"; task_id: string; millis: number }
  | { type: "failed"; task_id: string; error: string; millis: number }
  | { type: "pool";
      running: number;
      queued: number;
      completed: number;
      failed: number;
      bytes_written: number;
      rows_written: number;
      wall_ms: number };

let _progressUnlisten: UnlistenFn | null = null;

/** Start listening for backend worker events. Idempotent. */
export async function startProgressListener() {
  if (_progressUnlisten) return;
  if (typeof window === "undefined") return;
  if (!("__TAURI_INTERNALS__" in window) && !("__TAURI__" in window)) return;
  try {
    const { listen } = await import("@tauri-apps/api/event");
    _progressUnlisten = await listen<WorkerEvent>("tdds:progress", (e) => {
      const ev = e.payload;
      // Track in-flight set so the UI can show "running tasks" without
      // waiting on the 1.5 s SQL poll. Recent events also push into the
      // activity log so the console feels live.
      if (ev.type === "started") {
        app.runningTaskIds = [...app.runningTaskIds.filter((id) => id !== ev.task_id), ev.task_id];
      } else if (ev.type === "done" || ev.type === "empty" || ev.type === "failed") {
        app.runningTaskIds = app.runningTaskIds.filter((id) => id !== ev.task_id);
      }
      // Trigger an immediate poll so the UI snapshot picks up the new
      // SQLite state without waiting for the next tick.
      void _pollOnce();
    });
  } catch (e) {
    log("warn", `progress listener failed: ${e instanceof Error ? e.message : String(e)}`);
  }
}

export function stopProgressListener() {
  if (_progressUnlisten) {
    _progressUnlisten();
    _progressUnlisten = null;
  }
}

// ── Warm caches on connect ───────────────────────────────────
//
// Pre-load list endpoints so the Composer / SymbolPicker autocompletes
// open instantly. Failures are logged but never surface to the UI.
export async function warmCaches() {
  if (app.symbols.loading) return;
  app.symbols.loading = true;
  log("info", "Warming symbol caches");
  try {
    const [stocks, roots] = await Promise.all([
      api.listQuery({ endpoint: "stock_list_symbols", args: {} }).catch(() => []),
      api.listQuery({ endpoint: "option_list_symbols", args: {} }).catch(() => []),
    ]);
    app.symbols.stockSymbols = stocks.sort();
    app.symbols.optionRoots = roots.sort();
    app.symbols.loadedAt = Date.now();
    log("info", `Warm cache loaded`, {
      stocks: stocks.length,
      option_roots: roots.length,
    });
  } catch (e) {
    log("warn", `Warm cache failed: ${e instanceof Error ? e.message : String(e)}`);
  } finally {
    app.symbols.loading = false;
  }
}

// ── Convenience proxies (so existing imports keep working) ───
//
// We expose the same `currentView` / `detailDataset` etc. names as
// getter-objects backed by the single `app` $state. Components that
// imported these names previously work without code changes — but
// reassignments must go through `app.<field>` directly. To keep the
// migration ergonomic the store also exposes setter helpers.
export const currentView = {
  get value() { return app.currentView; },
};
export const detailDataset = {
  get value() { return app.detailDataset; },
};
export const railCollapsed = {
  get value() { return app.railCollapsed; },
};
export const queueSnap = {
  get value() { return app.queueSnap; },
};
export const connState = {
  get value() { return app.connState; },
};
export const connMsg = {
  get value() { return app.connMsg; },
};
export const composer = app.composer;
export const settings = app.settings;

// ── Navigation helpers ───────────────────────────────────────
export function navigate(v: View) {
  app.currentView = v;
  if (v !== "detail") app.detailDataset = null;
}
/** Open Browse already pointed at `kind` (and optionally a symbol).
 *
 *  Browse owns its step state locally, so a caller cannot reach in and
 *  set it; it hands over an intent instead and Browse applies it. The
 *  Library's "Download more dates" used to call a helper that took a
 *  `kind` and ignored it, dropping the user on whatever Browse had
 *  selected last. */
export function browseTo(kind: string, symbol = "") {
  app.browseIntent = { kind, symbol };
  navigate("browse");
}

/** Present one live catalogue entry in the shape the composer and the
 *  dataset cards take.
 *
 *  `DATASETS` below is a seven-entry mirror written before the
 *  catalogue was registry-driven; it still backs the home page's
 *  curated picks, but anything that needs to address *any* dataset has
 *  to go through the catalogue or it silently covers a ninth of the
 *  product. */
export function datasetFromCatalogue(entry: CatalogueEntry): DatasetMeta {
  const assetClass: AssetClass = entry.name.startsWith("option_")
    ? "option"
    : entry.name.startsWith("index_")
      ? "index"
      : entry.name.startsWith("rate_") || entry.name.startsWith("interest_")
        ? "rate"
        : "stock";
  return {
    id: entry.name,
    title: entry.summary || entry.name,
    subtitle: entry.description || "",
    assetClass,
    cadence: (entry.subcategory || "history") as Cadence,
    specLine: entry.rest_path,
    featured: false,
    tags: [entry.category, entry.subcategory].filter(Boolean),
  };
}

/** The catalogue entry for `name`, as a `DatasetMeta`. */
export function datasetById(name: string): DatasetMeta | null {
  const entry = app.catalogue.find((e) => e.name === name);
  return entry ? datasetFromCatalogue(entry) : null;
}

export function openDetail(d: DatasetMeta) {
  app.detailDataset = d;
  app.currentView = "detail";
}

// ── Queue polling ────────────────────────────────────────────
//
// Adaptive, because a fixed 1.5s poll of a 500-row snapshot plus a
// disk walk is real work to do forever in an app that is idle almost
// all of the time. Three rates:
//
//   ACTIVE  something is running or pending — the user is watching
//           progress bars, so stay responsive
//   IDLE    queue is drained; poll only to notice work arriving from
//           a schedule tick or a second window
//   HIDDEN  window is not on screen; nobody can see the result
//
// Self-scheduling `setTimeout` rather than `setInterval` so the rate
// can change between ticks, and so a slow snapshot cannot stack up
// overlapping calls the way a fixed interval can.
const POLL_ACTIVE_MS = 1500;
const POLL_IDLE_MS = 8000;
const POLL_HIDDEN_MS = 30000;

let _pollTimer: ReturnType<typeof setTimeout> | null = null;
let _pollRunning = false;

export function startQueuePoll() {
  if (_pollRunning) return;
  _pollRunning = true;
  app.queuePollActive = true;
  if (typeof document !== "undefined") {
    document.addEventListener("visibilitychange", _onVisibility);
  }
  void _pollLoop();
}

export function stopQueuePoll() {
  _pollRunning = false;
  if (_pollTimer !== null) {
    clearTimeout(_pollTimer);
    _pollTimer = null;
  }
  if (typeof document !== "undefined") {
    document.removeEventListener("visibilitychange", _onVisibility);
  }
  app.queuePollActive = false;
}

/** Coming back to the window should show current state immediately,
 *  not up to `POLL_HIDDEN_MS` later. */
function _onVisibility() {
  if (!_pollRunning || document.visibilityState !== "visible") return;
  if (_pollTimer !== null) clearTimeout(_pollTimer);
  void _pollLoop();
}

async function _pollLoop() {
  if (!_pollRunning) return;
  await _pollOnce();
  if (!_pollRunning) return;
  _pollTimer = setTimeout(() => void _pollLoop(), _pollDelay());
}

function _pollDelay(): number {
  if (typeof document !== "undefined" && document.visibilityState === "hidden") {
    return POLL_HIDDEN_MS;
  }
  return _queueBusy() ? POLL_ACTIVE_MS : POLL_IDLE_MS;
}

function _queueBusy(): boolean {
  return _busy(app.queueSnap);
}

function _busy(snap: QueueSnapshot | null): boolean {
  return (snap?.counts ?? []).some(
    ([status, n]) => n > 0 && (status === "running" || status === "pending"),
  );
}

/** Counts at the moment the queue last went from idle to busy, so the
 *  completion notice can report the run rather than the lifetime
 *  totals of the database. */
let _runBaseline: Record<string, number> | null = null;

async function _pollOnce() {
  try {
    const snap = await api.snapshot();
    const finishedBefore = _finishedCount(app.queueSnap);
    const wasBusy = _busy(app.queueSnap);
    app.queueSnap = snap;
    // A task that just finished changed what is on disk. Coverage was
    // fetched once per view on mount, so until this the Library and the
    // Home dashboard kept showing pre-download numbers until the user
    // navigated away and back.
    if (_finishedCount(snap) > finishedBefore) void loadCoverage(true);

    const isBusy = _busy(snap);
    if (isBusy && !wasBusy) {
      _runBaseline = _countMap(snap);
    } else if (!isBusy && wasBusy) {
      _announceRun(snap);
      _runBaseline = null;
    }
  } catch {
    // pre-connect; silently drop
  }
}

function _countMap(snap: QueueSnapshot): Record<string, number> {
  return Object.fromEntries(snap.counts);
}

/** Tell the user a run is over. A download queue is something you start
 *  and walk away from, so the one moment worth interrupting for is the
 *  moment it stops. */
function _announceRun(snap: QueueSnapshot) {
  const end = _countMap(snap);
  const start = _runBaseline ?? {};
  const delta = (k: string) => Math.max(0, (end[k] ?? 0) - (start[k] ?? 0));
  const done = delta("done");
  const failed = delta("failed");
  const empty = delta("empty");
  if (done + failed + empty === 0) return;

  const parts = [`${done} downloaded`];
  if (empty > 0) parts.push(`${empty} with no data`);
  if (failed > 0) parts.push(`${failed} failed`);
  const body = parts.join(", ");
  log(failed > 0 ? "warn" : "info", `Queue finished — ${body}`);
  if (app.settings.preferences?.notify_on_complete ?? true) {
    void notify("Downloads finished", body);
  }
}

function _finishedCount(snap: QueueSnapshot | null): number {
  if (!snap) return 0;
  return snap.counts
    .filter(([status]) => status === "done" || status === "empty" || status === "failed")
    .reduce((sum, [, n]) => sum + n, 0);
}

/** Refresh what is on disk. `force` re-fetches even when a copy is
 *  already held; without it the call is a no-op once loaded, which is
 *  what view mounts want. */
export async function loadCoverage(force = false) {
  if (app.coverageLoading) return;
  if (!force && app.coverage.length > 0) return;
  app.coverageLoading = true;
  try {
    app.coverage = await api.coverage();
  } catch {
    // not connected yet
  } finally {
    app.coverageLoading = false;
  }
}

/** Pull a fresh snapshot now. Row actions call this so the list reflects
 *  the change on the click rather than up to a poll interval later. */
export async function refreshQueueSnapshot() {
  await _pollOnce();
}

// ── Connection ───────────────────────────────────────────────
export async function connect() {
  app.connState = "connecting";
  app.connMsg = "Connecting…";
  try {
    await api.connect();
    app.connState = "connected";
    app.connMsg = "Connected to ThetaData";
    log("info", "Connected to ThetaData");
    startQueuePoll();
    // Fire-and-forget cache warmup so the UI is responsive immediately.
    void warmCaches();
    // Refresh subscription-tier gating now that auth captured tiers.
    void refreshTierStatus();
    // Load the YAML-driven endpoint catalogue.
    void loadCatalogue();
  } catch (e: unknown) {
    app.connState = "error";
    app.connMsg = e instanceof Error ? e.message : String(e);
    log("error", `Connect failed: ${app.connMsg}`);
    throw e;
  }
}

// ── Subscription-tier gating ─────────────────────────────────
//
// Pulled from `tier_status` + `tier_endpoints` after connect. The store
// holds (a) per-asset-class tier labels for the header badge and (b) per
// endpoint allowed/required verdicts so dataset cards can grey out gated
// rows. `tierForKind` covers the seven curated `DataKind`s using the
// client-side mirror in `api.ts` when the live verdict list isn't loaded
// yet (typically the first ~150ms after connect).

export async function refreshTierStatus() {
  try {
    const [status, verdicts] = await Promise.all([
      api.tierStatus(),
      api.tierEndpoints(),
    ]);
    app.tierStatus = status;
    app.tierVerdicts = verdicts;
    log("info", "Subscription tiers loaded", {
      stock: status.stock,
      options: status.options,
    });
  } catch (e) {
    log("warn", `tier_status failed: ${e instanceof Error ? e.message : String(e)}`);
  }
}

/** Fetch the YAML-driven endpoint catalogue from the backend and cache it
 *  in `app.catalogue`. Idempotent — skips the call if already loaded. */
export async function loadCatalogue() {
  if (app.catalogueLoading) return;
  app.catalogueLoading = true;
  try {
    const entries = await api.datasetCatalogue();
    app.catalogue = entries;
    log("info", `Catalogue loaded: ${entries.length} endpoints`);
  } catch (e) {
    log("warn", `Catalogue load failed: ${e instanceof Error ? e.message : String(e)}`);
  } finally {
    app.catalogueLoading = false;
  }
}

/** Verdict for a dataset by name, legacy spellings included.
 *
 *  Three sources, in order of authority: the backend's per-endpoint
 *  verdicts, the catalogue's `min_tier` (parsed from ThetaData's own
 *  `x-min-subscription`), and — only in the ~150ms before either has
 *  loaded — the static mirror in `api.ts`. Consulting the live verdict
 *  first matters: the mirror covers a handful of endpoints, so ranking
 *  it first reported almost every dataset as ungated and let the user
 *  queue downloads their subscription would refuse. */
export function tierForKind(kind: string): {
  required: TierName;
  user: TierName;
  allowed: boolean;
} {
  const endpoint = resolveKindToEndpoint(kind);
  const status = app.tierStatus;
  const userTier: TierName = status
    ? status[governingTierForKind(endpoint)]
    : "Unknown";

  const live = tierForEndpoint(endpoint);
  if (live) {
    return { required: live.required, user: live.user, allowed: live.allowed };
  }
  const fromCatalogue = app.catalogue.find((e) => e.name === endpoint)?.min_tier;
  const required = fromCatalogue ?? minTierForKind(endpoint);
  return { required, user: userTier, allowed: tierMeets(userTier, required) };
}

/** Verdict for a registered endpoint by name. */
export function tierForEndpoint(name: string): TierVerdict | null {
  return app.tierVerdicts.find((v) => v.endpoint === name) ?? null;
}

/** True iff the user's tier is high enough to ALSO unlock anything in
 *  `target`. Used by the header CTA to decide whether to render an
 *  "Upgrade" button at all. */
export function hasAnyGatedDatasets(): boolean {
  if (!app.tierStatus) return false;
  // If either tier is at Pro the user has the maximum bundle for that
  // class — but they may still be missing the OTHER class. Show the CTA
  // unless BOTH classes are at Pro.
  return TIER_RANK[app.tierStatus.stock] < TIER_RANK.Pro
      || TIER_RANK[app.tierStatus.options] < TIER_RANK.Pro;
}

// ── Composer helpers ─────────────────────────────────────────
// ── Per-dataset output format ────────────────────────────────────
//
// The Detail view offers a default format per dataset. It is stored
// here and applied whenever that dataset is queued, so the setting is
// the promise it makes rather than a control bound to nothing.

/** The formats the writer supports. Anything else is not renderable. */
export type OutputFormat = "parquet" | "csv" | "jsonl" | "json";
const OUTPUT_FORMATS: OutputFormat[] = ["parquet", "csv", "jsonl", "json"];

const DATASET_FORMAT_KEY = "tdds.datasetFormats";
let datasetFormats = $state<Record<string, OutputFormat>>({});

/** Load the remembered formats. Called once on launch. */
export async function loadDatasetFormats() {
  const stored = (await kvGet<Record<string, string>>(DATASET_FORMAT_KEY)) ?? {};
  // A hand-edited or downgraded store could hold anything; keep only
  // what the writer can actually produce.
  datasetFormats = Object.fromEntries(
    Object.entries(stored).filter(([, v]) => isOutputFormat(v)),
  ) as Record<string, OutputFormat>;
}

function isOutputFormat(value: string): value is OutputFormat {
  return (OUTPUT_FORMATS as string[]).includes(value);
}

export function datasetFormatFor(datasetId: string): OutputFormat {
  return datasetFormats[datasetId] ?? "parquet";
}

export function rememberDatasetFormat(datasetId: string, format: string) {
  if (!isOutputFormat(format)) return;
  datasetFormats = { ...datasetFormats, [datasetId]: format };
  void kvSet(DATASET_FORMAT_KEY, datasetFormats);
}

export function openComposer(dataset: DatasetMeta | null) {
  app.composer.anchorDataset = dataset;
  // Open on the format this dataset was last saved with, so the quick
  // add and the composer agree with the Detail view's setting.
  if (dataset) app.composer.format = datasetFormatFor(dataset.id);
  app.composer.open = true;
  app.composer.status = "idle";
  app.composer.msg = "";
}

export function closeComposer() {
  app.composer.open = false;
}

// ── Index preset modal helper ────────────────────────────────
import type { IndexPresetView } from "$lib/api";

export function openIndexPreset(p: IndexPresetView) {
  app.presetSelected = {
    id: p.id,
    name: p.name,
    description: p.description,
    symbols: p.symbols ?? [],
  };
  app.presetOpen = true;
}
export function closeIndexPreset() {
  app.presetOpen = false;
}

/** Open the one-shot dispatcher on `name`, pre-filling whatever
 *  arguments the caller already knows.
 *
 *  List endpoints (`option_list_expirations`, `stock_list_dates`, …)
 *  answer a question rather than produce a per-day dataset: they take
 *  no date range, so there is nothing for the queue to fan out over
 *  and `enqueue` rejects them outright. They belong here instead. */
export function openEndpointRunner(
  name: string,
  args: Record<string, string> = {},
  format: "parquet" | "csv" | "jsonl" | "json" = "csv",
) {
  const endpoint = app.catalogue.find((e) => e.name === name);
  if (!endpoint) {
    log("error", `Unknown endpoint ${name}`);
    return;
  }
  app.endpointRunner = {
    endpoint: {
      name: endpoint.name,
      description: endpoint.description,
      category: endpoint.category,
      subcategory: endpoint.subcategory,
      rest_path: endpoint.rest_path,
      returns: endpoint.returns,
      params: endpoint.params,
    },
    args,
    format,
    busy: false,
    msg: "",
  };
  app.endpointRunnerOpen = true;
}

// ── Settings ─────────────────────────────────────────────────
export async function loadSettings() {
  try {
    const s = await api.settingsGet();
    app.settings.db_path = s.db_path;
    app.settings.output_dir = s.output_dir;
    app.settings.creds_path = s.creds_path;
  } catch {
    /* ignore */
  }
}

// ── Dataset catalogue (YAML-driven, loaded from backend) ─────────
//
// `app.catalogue` is the live source of truth, populated by `loadCatalogue()`
// on connect. The static DATASETS constant below is kept as a minimal fallback
// for the AddModal composer anchor type only — it carries no schemaFields.
export const DATASETS: DatasetMeta[] = [
  { id: "option_trade_quote", title: "Option Trade-Quote", subtitle: "Every trade paired with its NBBO quote at execution", assetClass: "option", cadence: "trade_quote", specLine: "Full chain · tick-by-tick · parquet/csv/jsonl", featured: true,  tags: ["options", "NBBO", "tick"] },
  { id: "option_trade",       title: "Option Trade",       subtitle: "Every option trade across the full contract chain",   assetClass: "option", cadence: "trade",       specLine: "Full chain · tick-by-tick · parquet/csv/jsonl", featured: true,  tags: ["options", "tick"] },
  { id: "option_quote",       title: "Option Quote",       subtitle: "Every NBBO update for the full option chain",         assetClass: "option", cadence: "quote",       specLine: "Full chain · tick-by-tick or sampled · parquet/csv/jsonl", featured: false, tags: ["options", "NBBO", "quote"] },
  { id: "option_oi",          title: "Option Open Interest", subtitle: "End-of-day open interest snapshot for every strike", assetClass: "option", cadence: "oi",         specLine: "Full chain · daily EOD · parquet/csv/jsonl", featured: false, tags: ["options", "OI", "EOD"] },
  { id: "stock_trade",        title: "Stock Trade",        subtitle: "Every equity trade — full NMS tape",                  assetClass: "stock",  cadence: "trade",       specLine: "All NMS exchanges · tick-by-tick · parquet/csv/jsonl", featured: true,  tags: ["stocks", "tick", "NMS"] },
  { id: "stock_quote",        title: "Stock Quote",        subtitle: "Every NBBO update for equities",                      assetClass: "stock",  cadence: "quote",       specLine: "All NMS exchanges · tick-by-tick or sampled · parquet/csv/jsonl", featured: false, tags: ["stocks", "NBBO", "quote"] },
  { id: "stock_trade_quote",  title: "Stock Trade-Quote",  subtitle: "Every equity trade paired with the NBBO at execution", assetClass: "stock", cadence: "trade_quote", specLine: "All NMS exchanges · tick-by-tick · parquet/csv/jsonl", featured: true,  tags: ["stocks", "NBBO", "tick"] },
];

// ── Curated bundles ───────────────────────────────────────────
//
// Each bundle is a named preset that queues multiple kinds in a single
// click. The required tier is the MAX tier across member kinds, computed
// at runtime via TIER_RANK so it stays correct when tier mappings change.
export interface Bundle {
  id: string;
  title: string;
  description: string;
  kinds: string[];
  /** Optional: if non-null, restricts the bundle to this exact symbol. */
  lockedSymbol: string | null;
  assetClass: AssetClass;
}

export const BUNDLES: Bundle[] = [
  {
    id: "stock_complete",
    title: "Stock Complete",
    description: "Full tick record + EOD bars for equities",
    kinds: ["stock_history_trade", "stock_history_quote", "stock_history_eod"],
    lockedSymbol: null,
    assetClass: "stock",
  },
  {
    id: "stock_ohlc_eod",
    title: "Stock OHLC + EOD",
    description: "Intraday bars and end-of-day close",
    kinds: ["stock_history_ohlc", "stock_history_eod"],
    lockedSymbol: null,
    assetClass: "stock",
  },
  {
    id: "option_chain",
    title: "Option Chain",
    description: "Full chain trade + quote + open interest",
    kinds: ["option_history_trade", "option_history_quote", "option_history_open_interest"],
    lockedSymbol: null,
    assetClass: "option",
  },
  {
    id: "option_chain_greeks",
    title: "Option Chain + Greeks",
    description: "Chain bundle plus implied vol and first-order Greeks",
    kinds: [
      "option_history_trade",
      "option_history_quote",
      "option_history_open_interest",
      "option_history_greeks_implied_volatility",
      "option_history_greeks_first_order",
    ],
    lockedSymbol: null,
    assetClass: "option",
  },
  {
    id: "greeks_full",
    title: "Greeks Pack (Full)",
    description: "Implied vol + 1st / 2nd / 3rd order Greeks",
    kinds: [
      "option_history_greeks_implied_volatility",
      "option_history_greeks_first_order",
      "option_history_greeks_second_order",
      "option_history_greeks_third_order",
    ],
    lockedSymbol: null,
    assetClass: "option",
  },
  {
    id: "qqq_complete",
    title: "QQQ Complete",
    description: "Stock trade-quote + full option chain for QQQ",
    kinds: ["stock_history_trade_quote", "option_history_trade", "option_history_quote"],
    lockedSymbol: "QQQ",
    assetClass: "stock",
  },
  {
    id: "index_daily",
    title: "Index Daily",
    description: "Daily EOD and OHLC bars for index symbols",
    kinds: ["index_history_eod", "index_history_ohlc"],
    lockedSymbol: null,
    assetClass: "index",
  },
];

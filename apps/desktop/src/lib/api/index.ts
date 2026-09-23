// Thin Tauri invoke wrappers. Centralized so we can swap to
// tauri-plugin-conduit (faster IPC) by changing one import.
//
// In a plain browser preview (e.g. served via `vite preview` over Tailscale)
// the Tauri runtime is absent. We detect that case and short-circuit every
// command with a friendly error so the UI degrades gracefully instead of
// crashing on `Cannot read properties of undefined (reading 'invoke')`.
import { invoke as tauriInvoke } from "@tauri-apps/api/core";

const isTauri =
  typeof window !== "undefined" &&
  // Tauri 2 exposes both __TAURI_INTERNALS__ and __TAURI__.
  (("__TAURI_INTERNALS__" in window) || ("__TAURI__" in window));

export const TAURI_AVAILABLE = isTauri;

function noTauri(): never {
  throw new Error(
    "Browser preview mode — TdDx Store backend not available. Run the desktop app (`npm run tauri dev`) to use real ThetaData calls.",
  );
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) noTauri();
  return tauriInvoke<T>(cmd, args);
}

export type Watchlist = { name: string; symbols: string[] };

/** How the user works. Pre-fills and tunes; never blocks a choice. */
export type Preferences = {
  default_format: "parquet" | "csv" | "jsonl" | "json";
  default_asset_class: "stock" | "option" | "index" | "rate";
  default_dataset: string | null;
  default_range_years: number;
  watchlists: Watchlist[];
  /** Fewer concurrent downloads than the plan allows; null = whole budget. */
  max_concurrency: number | null;
  notify_on_complete: boolean;
  split_failed_windows: boolean;
};

export const DEFAULT_PREFERENCES: Preferences = {
  default_format: "parquet",
  default_asset_class: "stock",
  default_dataset: null,
  default_range_years: 3,
  watchlists: [],
  max_concurrency: null,
  notify_on_complete: true,
  split_failed_windows: true,
};

export type Settings = {
  db_path: string;
  output_dir: string;
  creds_path: string;
  email?: string;
  password?: string;
  preferences: Preferences;
};

/** Tagged to match the Rust `LoginArgs` enum, so "an API key plus a
 *  blank password" is not representable on either side. */
export type LoginArgs =
  | { method: "password"; email: string; password: string }
  | { method: "api_key"; api_key: string; email?: string | null };

export type Counts = [string, number][];

export type TaskView = {
  id: string;
  status: "pending" | "running" | "done" | "failed" | "empty" | "paused";
  kind: string;
  symbol: string;
  date: string;
  /** End of the window, for a task that covers a range. `null` when the
   *  task is one session. */
  end_date: string | null;
  rows: number | null;
  bytes: number | null;
  error: string | null;
  attempts: number;
  /** Where the task writes. Present so "open file location" has a path
   *  without the UI reconstructing the naming scheme. */
  path: string;
};

export type QueueSnapshot = {
  counts: Counts;
  recent: TaskView[];
  bytes_on_disk: number;
  files_on_disk: number;
};

export type Coverage = {
  kind: string;
  symbol: string;
  files: number;
  bytes: number;
  first: string | null;
  last: string | null;
  /** Every date on disk, ISO `YYYY-MM-DD`. The span alone cannot show
   *  gaps, which is the whole point of a coverage view. */
  dates: string[];
  /** The file holding the latest date, to open without guessing a name. */
  latest_path: string | null;
  /** Extension the existing files use, so a refill writes the same
   *  format as the rest of the set. */
  format: string;
};

export type Transforms = {
  scale?: Record<string, number>;
  rename?: Record<string, string>;
  drop?: string[];
};

/** What a selection becomes once the app has absorbed the server's
 *  constraints — the window cap, the per-day endpoint shape, and the
 *  endpoints that refuse an expiration wildcard. */
/** One download as the user asked for it, rolled up from its tasks. */
export type Batch = {
  id: string;
  kind: string;
  symbols: number;
  first_symbol: string;
  start: string;
  end: string;
  format: string;
  total: number;
  pending: number;
  running: number;
  paused: number;
  done: number;
  empty: number;
  failed: number;
  /** Failed tasks re-queued as narrower windows; their work continues
   *  in their halves, so they count toward neither progress nor failure. */
  split: number;
  rows: number;
  bytes: number;
  created_at: number;
  /** First task picked up; null until one has been. */
  started_at: number | null;
  finished_at: number | null;
  /** Mean seconds per finished task, measured on this batch. */
  avg_task_secs: number | null;
};

/** One file summarised for a chart; see `tdds_core::chart`. */
export type ChartSeries = {
  shape: "candles" | "line" | "band";
  rows: number;
  daily: boolean;
  x: number[];
  open: (number | null)[];
  high: (number | null)[];
  low: (number | null)[];
  close: (number | null)[];
  bid: (number | null)[];
  ask: (number | null)[];
  volume: (number | null)[];
  gaps: { from_ms: number; to_ms: number }[];
};

export type EnqueuePlan = {
  requests: number;
  windows: number;
  expirations: number;
};

export type EnqueueArgs = {
  kind: string;
  symbol: string;
  date?: string | null;
  start?: string | null;
  end?: string | null;
  format: string;
  interval?: string | null;
  expiration?: string | null;
  strike?: string | null;
  right?: string | null;
  priority?: number | null;
  transforms?: Transforms | null;
  /** Any other parameter the endpoint declares — `max_dte`,
   *  `strike_range`, `start_time`, the greeks inputs. Keyed by registry
   *  param name; keys the endpoint does not declare are dropped when
   *  the task is lowered onto a request, not rejected. */
  extra?: Record<string, string> | null;
  /** One id for a whole submission, so ten symbols queued together read
   *  as one download. */
  batch_id?: string | null;
};

export type EndpointParam = {
  name: string;
  description: string;
  param_type: string;
  required: boolean;
};

export type EndpointInfo = {
  name: string;
  description: string;
  category: string;
  subcategory: string;
  rest_path: string;
  returns: string;
  params: EndpointParam[];
};

export type CatalogueEntry = {
  name: string;
  category: string;
  subcategory: string;
  rest_path: string;
  returns: string;
  params: EndpointParam[];
  summary: string;
  description: string;
  tag: string;
  min_tier: TierName | null;
};

export type EndpointMeta = {
  operation_id: string;
  summary: string;
  description: string;
  tag: string;
  min_tier: TierName | null;
};

export type InvokeArgs = {
  endpoint: string;
  args: Record<string, string>;
  format: string;
  output_path: string;
  timeout_ms?: number | null;
};

export type ListQueryArgs = {
  endpoint: string;
  args: Record<string, string>;
  timeout_ms?: number | null;
};

export const api = {
  settingsGet: () => invoke<Settings>("settings_get"),
  settingsSet: (settings: Settings) => invoke<void>("settings_set", { settings }),
  logout: () => invoke<void>("logout"),
  connect: () => invoke<string>("connect"),
  login: (args: LoginArgs) => invoke<string>("login", { args }),
  /** What `enqueue` would queue, without queueing it. Same code path,
   *  so the number shown is the number that happens. */
  estimate: (args: EnqueueArgs) => invoke<EnqueuePlan>("estimate", { args }),
  batches: () => invoke<Batch[]>("batches"),
  chartSeries: (path: string) => invoke<ChartSeries>("chart_series", { path }),
  pauseBatch: (id: string) => invoke<number>("pause_batch", { id }),
  resumeBatch: (id: string) => invoke<number>("resume_batch", { id }),
  removeBatch: (id: string) => invoke<number>("remove_batch", { id }),
  enqueue: (args: EnqueueArgs) => invoke<number>("enqueue", { args }),
  snapshot: () => invoke<QueueSnapshot>("snapshot"),
  coverage: () => invoke<Coverage[]>("coverage_report"),
  /** Returns true iff a new pool was started; false means one is already running. */
  runQueue: () => invoke<boolean>("run_queue"),
  requeueFailed: () => invoke<number>("requeue_failed"),
  /** Bulk queue operations. Each returns how many rows it changed;
   *  rows that had already moved on are skipped rather than erroring. */
  cancelTasks: (ids: string[]) => invoke<number>("cancel_tasks", { ids }),
  requeueTasks: (ids: string[]) => invoke<number>("requeue_tasks", { ids }),
  /** Deletes rows outright, live ones included — no cancel-first step. */
  removeTasks: (ids: string[]) => invoke<number>("remove_tasks", { ids }),
  bumpTasks: (ids: string[]) => invoke<number>("bump_tasks", { ids }),
  duplicateTasks: (ids: string[]) => invoke<number>("duplicate_tasks", { ids }),
  /** Delete every finished row, or every row in one finished status.
   *  Runs against the whole queue, not just the loaded page. */
  clearTasks: (status?: "done" | "failed" | "empty") =>
    invoke<number>("clear_tasks", { status: status ?? null }),
  workerPoolActive: () => invoke<boolean>("worker_pool_active"),
  health: () => invoke<HealthSnapshot>("health"),
  /** Tauri 2 maps a Rust `output_dir` parameter to the camelCase key
   *  `outputDir` on the JS side; passing the snake_case key is rejected
   *  as a missing argument. */
  duckdbCommand: (outputDir: string) =>
    invoke<{ sql: string; path: string; hint: string }>("duckdb_command", { outputDir }),
  endpointsList: () => invoke<EndpointInfo[]>("endpoints_list"),
  /** Intervals this dataset accepts. Omitting the kind returns them all,
   *  which is only correct for a picker not attached to a dataset. */
  intervalOptions: (kind?: string) =>
    invoke<IntervalOption[]>("interval_options", { kind: kind ?? null }),
  endpointInvoke: (args: InvokeArgs) => invoke<number>("endpoint_invoke", { args }),
  listQuery: (args: ListQueryArgs) => invoke<string[]>("list_query", { args }),
  flatfileDatasets: () => invoke<FlatfileDataset[]>("flatfile_datasets"),
  /** The exact trading days absent from a set's span, `YYYY-MM-DD`.
   *  Read-only; the count matches what `requeueMissingDates` would act
   *  on, so the UI never shows a number the server disagrees with. */
  missingDates: (kind: string, symbol: string) =>
    invoke<string[]>("missing_dates", { kind, symbol }),
  requeueMissingDates: (kind: string, symbol: string) =>
    invoke<number>("requeue_missing_dates", { kind, symbol }),
  indexPresets: () => invoke<IndexPresetView[]>("index_presets"),
  indexConstituents: (indexId: string) => invoke<string[]>("index_constituents", { indexId }),
  parquetPreview: (args: ParquetPreviewArgs) => invoke<PreviewResult>("parquet_preview", { args }),
  scheduleList: () => invoke<ScheduleRow[]>("schedule_list"),
  scheduleCreate: (args: ScheduleCreateArgs) => invoke<ScheduleRow>("schedule_create", { args }),
  scheduleDelete: (id: string) => invoke<void>("schedule_delete", { id }),
  scheduleSetPaused: (id: string, paused: boolean) => invoke<void>("schedule_set_paused", { id, paused }),
  vaultPaths: () => invoke<{ vault_path: string; auto_password: string }>("vault_paths"),
  tierStatus: () => invoke<TierStatus>("tier_status"),
  tierEndpoints: () => invoke<TierVerdict[]>("tier_endpoints"),
  datasetCatalogue: () => invoke<CatalogueEntry[]>("dataset_catalogue"),
};

export type TierName = "Unknown" | "Free" | "Value" | "Standard" | "Pro";

export type ClassTier = {
  /** Wire name: `stock` | `option` | `index` | `rate`. */
  class: "stock" | "option" | "index" | "rate";
  /** Display label: `Stocks` | `Options` | `Indices` | `Rates`. */
  label: string;
  /** Tier name, already normalized by the backend (Unknown→Free post-connect). */
  tier: TierName;
  /** True when at the highest tier — hide the upgrade affordance. */
  at_max: boolean;
};

export type TierStatus = {
  stock: TierName;
  options: TierName;
  indices: TierName;
  interest_rate: TierName;
  /** Iterable per-class view. Authoritative for rendering — never
   *  derive class lists or Unknown-→-Free fallbacks on the FE. The
   *  backend already applied them. */
  classes: ClassTier[];
  /** Downloads in flight at once, for the whole account — the highest
   *  per-class `2^tier`, never the sum. */
  in_flight_budget: number;
  upgrade_url: string;
  connected: boolean;
};

export type TierVerdict = {
  endpoint: string;
  category: string;
  subcategory: string;
  required: TierName;
  user: TierName;
  allowed: boolean;
};

export const TIER_RANK: Record<TierName, number> = {
  Unknown: -1,
  Free: 0,
  Value: 1,
  Standard: 2,
  Pro: 3,
};

/** True iff `user` meets `required`. Mirrors `Tier::meets` server-side. */
export function tierMeets(user: TierName, required: TierName): boolean {
  return TIER_RANK[user] >= TIER_RANK[required];
}

/** Map a dataset name to the asset class whose subscription gates it.
 *  Mirrors `governing_tier` server-side for the case where the UI knows
 *  the name but not the full EndpointInfo. */
export function governingTierForKind(
  kind: string,
): "stock" | "options" | "indices" | "interest_rate" {
  const op = resolveKindToEndpoint(kind);
  if (op.startsWith("option_")) return "options";
  if (op.startsWith("index_")) return "indices";
  if (op.startsWith("rate_")) return "interest_rate";
  return "stock";
}

/** Authoritative client-side mirror of the per-endpoint tier table —
 *  baked from `docs.thetadata.us/openapiv3.yaml` `x-min-subscription`.
 *  Server stays the source of truth via `tier_endpoints`; this gives
 *  instant UI feedback before that command returns. Re-bake whenever
 *  ThetaData publishes a new spec. */
const TIER_TABLE: Record<string, TierName> = {
  // Stocks
  stock_list_symbols: "Free",
  stock_list_dates: "Free",
  stock_history_eod: "Free",
  stock_history_ohlc: "Value",
  stock_history_quote: "Value",
  stock_history_trade: "Standard",
  stock_history_trade_quote: "Standard",
  stock_snapshot_ohlc: "Value",
  stock_snapshot_trade: "Standard",
  stock_snapshot_quote: "Value",
  stock_snapshot_market_value: "Standard",
  stock_at_time_trade: "Standard",
  stock_at_time_quote: "Value",
  // Options
  option_list_symbols: "Free",
  option_list_dates: "Free",
  option_list_expirations: "Free",
  option_list_strikes: "Free",
  option_list_contracts: "Value",
  option_history_eod: "Free",
  option_history_ohlc: "Value",
  option_history_trade: "Standard",
  option_history_quote: "Value",
  option_history_trade_quote: "Standard",
  option_history_open_interest: "Value",
  option_history_greeks_eod: "Standard",
  option_history_greeks_implied_volatility: "Standard",
  option_history_greeks_first_order: "Standard",
  option_history_greeks_second_order: "Pro",
  option_history_greeks_third_order: "Pro",
  option_history_greeks_all: "Pro",
  option_history_trade_greeks_implied_volatility: "Pro",
  option_history_trade_greeks_first_order: "Pro",
  option_history_trade_greeks_second_order: "Pro",
  option_history_trade_greeks_third_order: "Pro",
  option_history_trade_greeks_all: "Pro",
  option_snapshot_ohlc: "Value",
  option_snapshot_trade: "Standard",
  option_snapshot_quote: "Value",
  option_snapshot_open_interest: "Value",
  option_snapshot_market_value: "Standard",
  option_snapshot_greeks_implied_volatility: "Standard",
  option_snapshot_greeks_first_order: "Standard",
  option_snapshot_greeks_second_order: "Pro",
  option_snapshot_greeks_third_order: "Pro",
  option_snapshot_greeks_all: "Pro",
  option_at_time_trade: "Standard",
  option_at_time_quote: "Value",
  // Indices
  index_list_symbols: "Free",
  index_list_dates: "Free",
  index_history_eod: "Free",
  index_history_ohlc: "Standard",
  index_history_price: "Value",
  index_snapshot_ohlc: "Standard",
  index_snapshot_price: "Standard",
  index_snapshot_market_value: "Standard",
  index_at_time_price: "Value",
  // Rates
  interest_rate_history_eod: "Value",
  // Calendar
  calendar_open_today: "Free",
  calendar_on_date: "Value",
  calendar_year: "Value",
};

/** Dataset names from before the queue and the catalogue shared one
 *  vocabulary. Kept so saved searches and schedules written by those
 *  builds still resolve; mirrors `LEGACY_ALIASES` in `tdds_core::spec`. */
const LEGACY_ALIASES: Record<string, string> = {
  stock_trade: "stock_history_trade",
  stock_quote: "stock_history_quote",
  stock_trade_quote: "stock_history_trade_quote",
  option_trade: "option_history_trade",
  option_quote: "option_history_quote",
  option_trade_quote: "option_history_trade_quote",
  option_oi: "option_history_open_interest",
};

/** Resolve any dataset name — current or legacy — to its endpoint. */
export function resolveKindToEndpoint(kind: string): string {
  return LEGACY_ALIASES[kind] ?? kind;
}

/** Last-resort tier guess, used only before the backend's verdicts land.
 *  The table below covers a handful of endpoints; anything missing is
 *  reported as `Unknown`, which gates rather than waving through. Do not
 *  call this directly — `tierForKind` consults the authoritative sources
 *  first. */
export function minTierForKind(kind: string): TierName {
  return TIER_TABLE[resolveKindToEndpoint(kind)] ?? "Unknown";
}

export type ParquetPreviewArgs = {
  path: string;
  offset?: number;
  limit?: number;
};

export type PreviewField = { name: string; dtype: string; nullable: boolean };
export type PreviewResult = {
  schema: PreviewField[];
  rows: unknown[][];
  total_rows: number;
  returned: number;
  bytes: number;
};

export type ScheduleRow = {
  id: string;
  name: string;
  kind: string;
  symbol: string;
  format: string;
  cron_kind: string;
  at_time: string;
  last_fired_at: number | null;
  paused: boolean;
  created_at: number;
};
export type ScheduleCreateArgs = {
  name: string;
  kind: string;
  symbol: string;
  format: string;
  cron_kind: string;
  at_time: string;
};

export type HealthSnapshot = {
  pool_size: number;
  workers_in_flight: number;
  pool_active: boolean;
  task_counts: Record<string, number>;
  total_files_on_disk: number;
  total_bytes_on_disk: number;
  uptime_secs: number;
  desktop_version: string;
  thetadatadx_version: string;
};

/** The `sec_type` values the flat-file distribution serves. Index is
 *  absent on purpose: there is no index flat file. */
/** One sampling interval the endpoints accept: `id` is the wire value,
 *  `label` is display copy. Sourced from `interval_options` — the UI
 *  never writes its own interval list. */
export type IntervalOption = {
  id: string;
  label: string;
};

export type FlatfileSecType = "OPTION" | "STOCK";
/** The `req_type` values the flat-file distribution serves. Per-tick
 *  trades, quotes and OHLC bars are market-data endpoints, not flat
 *  files. */
export type FlatfileReqType = "trade_quote" | "open_interest" | "eod";

/** One served `(sec_type, req_type)` pair, from the backend's copy of
 *  the SDK's `SERVED_DATASETS`. Never hardcode this list in the UI. */
export type FlatfileDataset = {
  sec_type: FlatfileSecType;
  req_type: FlatfileReqType;
  /** The dataset kind that queues it, e.g. `flatfile_option_trade_quote`. */
  kind: string;
};

/** A whole-market flat file has no symbol; this is what it is filed under. */
export const WHOLE_MARKET = "ALL";


export type IndexPresetView = {
  id: string;
  name: string;
  description: string;
  symbols: string[];
  as_of: string | null;
};

export const fmtBytes = (n: number | null | undefined) => {
  if (n == null) return "—";
  const u = ["B", "KB", "MB", "GB", "TB"];
  let i = 0; let v = n;
  while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
  return `${v.toFixed(i === 0 ? 0 : 1)} ${u[i]}`;
};
export const fmtNum = (n: number | null | undefined) =>
  n == null ? "—" : new Intl.NumberFormat().format(n);

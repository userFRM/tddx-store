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

export type Settings = {
  db_path: string;
  output_dir: string;
  creds_path: string;
  email?: string;
  password?: string;
};

export type LoginArgs = { email: string; password: string };

export type Counts = [string, number][];

export type TaskView = {
  id: string;
  status: "pending" | "running" | "done" | "failed" | "empty";
  kind: string;
  symbol: string;
  date: string;
  rows: number | null;
  bytes: number | null;
  error: string | null;
  attempts: number;
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
};

export type Transforms = {
  scale?: Record<string, number>;
  rename?: Record<string, string>;
  drop?: string[];
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
  enqueue: (args: EnqueueArgs) => invoke<number>("enqueue", { args }),
  snapshot: () => invoke<QueueSnapshot>("snapshot"),
  coverage: () => invoke<Coverage[]>("coverage_report"),
  /** Returns true iff a new pool was started; false means one is already running. */
  runQueue: () => invoke<boolean>("run_queue"),
  requeueFailed: () => invoke<number>("requeue_failed"),
  cancelTask: (id: string) => invoke<void>("cancel_task", { id }),
  workerPoolActive: () => invoke<boolean>("worker_pool_active"),
  health: () => invoke<HealthSnapshot>("health"),
  duckdbCommand: (output_dir: string) =>
    invoke<{ sql: string; path: string; hint: string }>("duckdb_command", { output_dir }),
  sdkVersion: () => invoke<{ thetadatadx: string; tdbe: string }>("sdk_version"),
  endpointsList: () => invoke<EndpointInfo[]>("endpoints_list"),
  endpointsGet: (name: string) => invoke<EndpointInfo>("endpoints_get", { name }),
  endpointInvoke: (args: InvokeArgs) => invoke<number>("endpoint_invoke", { args }),
  listQuery: (args: ListQueryArgs) => invoke<string[]>("list_query", { args }),
  flatfileDownload: (args: FlatfileArgs) => invoke<string>("flatfile_download", { args }),
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
  datasetMetadata: (name: string) => invoke<EndpointMeta>("dataset_metadata", { name }),
};

export type TierName = "Unknown" | "Free" | "Value" | "Standard" | "Pro";

export type ClassTier = {
  /** Wire name: `stock` | `option` | `index` | `rate`. */
  class: "stock" | "option" | "index" | "rate";
  /** Display label: `Stocks` | `Options` | `Indices` | `Rates`. */
  label: string;
  /** Tier name, already normalized by the backend (Unknown→Free post-connect). */
  tier: TierName;
  /** Parallel-download budget from `Tier::workers()`. */
  workers: number;
  /** True when at the highest tier — hide the upgrade affordance. */
  at_max: boolean;
};

export type TierStatus = {
  stock: TierName;
  options: TierName;
  indices: TierName;
  interest_rate: TierName;
  /** Iterable per-class view. Authoritative for rendering — never
   *  derive class lists, worker counts, or Unknown-→-Free fallbacks
   *  on the FE. The backend already applied them. */
  classes: ClassTier[];
  /** Sum of `classes[*].workers`. */
  total_workers: number;
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

/** Map a kind string ("stock_trade", "option_quote", …) to its governing
 *  tier ("stock" | "options"). Mirrors `governing_tier` server-side for
 *  the simple case where the UI knows the kind but not the full
 *  EndpointInfo. */
export function governingTierForKind(kind: string): "stock" | "options" {
  return kind.startsWith("option_") ? "options" : "stock";
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

/** Curated `DataKind` short ids in the BrowseView are aliases for the
 *  full operationId. Map them to the authoritative endpoint. */
const DATAKIND_TO_ENDPOINT: Record<string, string> = {
  stock_trade: "stock_history_trade",
  stock_quote: "stock_history_quote",
  stock_trade_quote: "stock_history_trade_quote",
  option_trade: "option_history_trade",
  option_quote: "option_history_quote",
  option_trade_quote: "option_history_trade_quote",
  option_oi: "option_history_open_interest",
};

export function minTierForKind(kind: string): TierName {
  const op = DATAKIND_TO_ENDPOINT[kind] ?? kind;
  return TIER_TABLE[op] ?? "Free";
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
  pool_per_class: { stock: number; option: number; index: number; rate: number };
  workers_in_flight: number;
  pool_active: boolean;
  task_counts: Record<string, number>;
  total_files_on_disk: number;
  total_bytes_on_disk: number;
  uptime_secs: number;
  desktop_version: string;
  thetadatadx_version: string;
  tdbe_version: string;
};

export type FlatfileArgs = {
  sec_type: "OPTION" | "STOCK" | "INDEX";
  req_type: "TRADE" | "QUOTE" | "TRADE_QUOTE" | "OPEN_INTEREST" | "OHLC" | "EOD";
  date: string;
  output_path: string;
  format: "CSV" | "JSONL";
};

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

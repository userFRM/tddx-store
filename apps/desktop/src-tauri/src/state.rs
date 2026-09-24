//! Shared application state + cross-command serializable types.
//!
//! Held inside `Arc<AppState>` and registered with `tauri::Builder::manage`.
//! Every Tauri command reads it through `State<'_, Arc<AppState>>`.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tdds_core::{Client, Queue, Task};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

/// Shared application state. We use `RwLock` for the queue + client +
/// settings holders so polling commands (snapshot, status badge) never
/// stall behind a long write held during the connect / login network
/// handshake. Locks are cloned out (the inner `Queue` and `Client` are
/// `Clone` via `Arc`) and dropped before any await — see the
/// "lock-then-clone" pattern used by every command.
#[derive(Default)]
pub struct AppState {
    pub queue: RwLock<Option<Queue>>,
    pub client: RwLock<Option<Client>>,
    pub settings: RwLock<Settings>,
    /// Last on-disk footprint reading, with the instant it was taken.
    /// The reading costs a full walk of the output tree, so snapshots
    /// and Health share it; the library watcher clears it on change.
    pub disk_usage: Mutex<Option<(std::time::Instant, DiskUsage)>>,
    /// Held by `commands::queue::run_queue` so a second click can't
    /// double-spawn workers. `JoinHandle` is `!Sync` only via inner
    /// state; `Mutex` keeps it safe across the await of `is_finished`.
    pub worker_handle: Mutex<Option<JoinHandle<()>>>,
    /// Set once in the setup hook, so state changes can be announced to
    /// the webview from anywhere that holds the state.
    pub app: std::sync::OnceLock<tauri::AppHandle>,
    /// The output-directory watcher. Replaced when the directory moves;
    /// dropping it stops the watch.
    pub watcher: std::sync::Mutex<
        Option<notify_debouncer_mini::Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>>,
    >,
}

// Path defaults intentionally derive to empty `String`s. They get
// seeded from `app.path().app_data_dir()` in the Tauri `setup()` hook
// so the app uses the OS-correct data dir (XDG on Linux, the
// sandboxed Application Support dir on macOS, %APPDATA% on Windows).
// Hardcoding `$HOME/tddx-store/...` is non-portable and breaks on
// platforms without `HOME`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub db_path: String,
    pub output_dir: String,
    /// Optional fallback if email/password not provided on connect.
    #[serde(default)]
    pub creds_path: String,
    /// In-memory only. Not serialized to disk via this struct — the
    /// vault owns persistence, and only on opt-in.
    #[serde(default, skip_serializing)]
    pub email: String,
    #[serde(default, skip_serializing)]
    pub password: String,
    /// API key issued from the account portal. Set when the user signed
    /// in with a key instead of email and password; mutually exclusive
    /// with the pair above.
    #[serde(default, skip_serializing)]
    pub api_key: String,
    /// How the user works, so the app stops asking. Persisted with the
    /// paths; `#[serde(default)]` so a settings file written before
    /// this existed still loads.
    #[serde(default)]
    pub preferences: Preferences,
}

/// Defaults and behaviour the user chooses. Every field pre-fills or
/// tunes something the user could otherwise set per download, so none
/// of it ever blocks a choice.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// `parquet` | `csv` | `jsonl` | `json`.
    pub default_format: String,
    /// `stock` | `option` | `index` | `rate`.
    pub default_asset_class: String,
    /// A registry endpoint name, or none to leave step 3 open.
    pub default_dataset: Option<String>,
    /// The range preset Browse starts on, in years.
    pub default_range_years: u32,
    /// Named symbol lists, offered as a symbol source in Browse.
    pub watchlists: Vec<Watchlist>,
    /// Run fewer downloads at once than the plan allows. The budget is
    /// account-wide, so this is how to leave headroom for other tools
    /// on the same account. `None` uses the whole budget.
    pub max_concurrency: Option<usize>,
    /// Show a system notification when a run finishes.
    pub notify_on_complete: bool,
    /// Re-queue a failed multi-day task as two halves.
    pub split_failed_windows: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            default_format: "parquet".into(),
            default_asset_class: "stock".into(),
            default_dataset: None,
            default_range_years: 3,
            watchlists: Vec::new(),
            max_concurrency: None,
            notify_on_complete: true,
            split_failed_windows: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Watchlist {
    pub name: String,
    pub symbols: Vec<String>,
}

/// Bytes and file count under the output directory.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiskUsage {
    pub bytes: u64,
    pub files: usize,
}

#[derive(Debug, Serialize)]
pub struct QueueSnapshot {
    pub counts: Vec<(String, i64)>,
    pub recent: Vec<TaskView>,
    pub bytes_on_disk: u64,
    pub files_on_disk: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct TaskView {
    pub id: String,
    pub status: String,
    pub kind: String,
    pub symbol: String,
    pub date: String,
    /// End of the window, when the task covers a range rather than one
    /// session. `None` for a single-day task.
    pub end_date: Option<String>,
    pub rows: Option<i64>,
    pub bytes: Option<i64>,
    pub error: Option<String>,
    pub attempts: i32,
    /// Where this task writes. Sent so "open file location" has a path
    /// to reveal without the UI reconstructing the naming scheme.
    pub path: String,
}

impl From<Task> for TaskView {
    fn from(t: Task) -> Self {
        let path = tdds_core::coverage::dataset_path(
            std::path::Path::new(&t.output_dir),
            &t.spec,
            t.format.extension(),
        );
        Self {
            id: t.id,
            status: t.status.as_str().to_string(),
            kind: t.spec.kind.as_str().to_string(),
            symbol: t.spec.symbol,
            date: t.spec.date.format("%Y-%m-%d").to_string(),
            end_date: t
                .spec
                .end_date
                .filter(|d| *d != t.spec.date)
                .map(|d| d.format("%Y-%m-%d").to_string()),
            rows: t.rows,
            bytes: t.bytes,
            error: t.error,
            attempts: t.attempts,
            path: path.to_string_lossy().into_owned(),
        }
    }
}

/// Parse `YYYYMMDD` or `YYYY-MM-DD` into a `NaiveDate`.
pub fn parse_ymd(s: &str) -> Result<NaiveDate, String> {
    let t = s.replace('-', "");
    NaiveDate::parse_from_str(&t, "%Y%m%d").map_err(|e| format!("bad date {s}: {e}"))
}

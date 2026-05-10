//! Shared application state + cross-command serializable types.
//!
//! Held inside `Arc<AppState>` and registered with `tauri::Builder::manage`.
//! Every Tauri command reads it through `State<'_, Arc<AppState>>`.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tdds_core::queue::TaskStatus;
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
    /// Held by `commands::queue::run_queue` so a second click can't
    /// double-spawn workers. `JoinHandle` is `!Sync` only via inner
    /// state; `Mutex` keeps it safe across the await of `is_finished`.
    pub worker_handle: Mutex<Option<JoinHandle<()>>>,
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
    /// In-memory only. Not serialized to disk via this struct.
    #[serde(default, skip_serializing)]
    pub email: String,
    #[serde(default, skip_serializing)]
    pub password: String,
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
    pub rows: Option<i64>,
    pub bytes: Option<i64>,
    pub error: Option<String>,
    pub attempts: i32,
}

impl From<Task> for TaskView {
    fn from(t: Task) -> Self {
        Self {
            id: t.id,
            status: status_str(t.status).to_string(),
            kind: t.spec.kind.as_str().to_string(),
            symbol: t.spec.symbol,
            date: t.spec.date.format("%Y-%m-%d").to_string(),
            rows: t.rows,
            bytes: t.bytes,
            error: t.error,
            attempts: t.attempts,
        }
    }
}

pub fn status_str(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Pending => "pending",
        TaskStatus::Running => "running",
        TaskStatus::Done => "done",
        TaskStatus::Failed => "failed",
        TaskStatus::Empty => "empty",
    }
}

/// Parse `YYYYMMDD` or `YYYY-MM-DD` into a `NaiveDate`.
pub fn parse_ymd(s: &str) -> Result<NaiveDate, String> {
    let t = s.replace('-', "");
    NaiveDate::parse_from_str(&t, "%Y%m%d").map_err(|e| format!("bad date {s}: {e}"))
}

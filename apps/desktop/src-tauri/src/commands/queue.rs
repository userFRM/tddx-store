//! Queue lifecycle: enqueue, snapshot, drain via worker pool, cancel,
//! requeue. The pool runs single-flight (a second `run_queue` while one
//! is in flight is a no-op) and forwards `ProgressEvent`s to the
//! webview as `tdds:progress` events.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::NaiveDate;
use serde::Deserialize;
use tauri::{Emitter, State};
use tdds_core::{
    coverage, format::OutputFormat, queue::TaskStatus, DataKind, DataSpec, Pool, ProgressEvent,
    Queue,
};

use crate::state::{parse_ymd, AppState, DiskUsage, QueueSnapshot, TaskView};

#[derive(Deserialize)]
pub struct EnqueueArgs {
    pub kind: String,
    pub symbol: String,
    pub date: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub format: String,
    pub interval: Option<String>,
    pub expiration: Option<String>,
    pub strike: Option<String>,
    pub right: Option<String>,
    pub priority: Option<i32>,
    #[serde(default)]
    pub transforms: Option<tdds_core::Transforms>,
    /// Any other registry parameter the endpoint declares — `max_dte`,
    /// `strike_range`, `start_time`, the greeks inputs. Keys the
    /// endpoint does not declare are ignored downstream.
    #[serde(default)]
    pub extra: Option<std::collections::BTreeMap<String, String>>,
}

#[tauri::command]
pub async fn enqueue(state: State<'_, Arc<AppState>>, args: EnqueueArgs) -> Result<usize, String> {
    let cfg = state.settings.read().await.clone();
    let queue_guard = state.queue.read().await;
    let queue = queue_guard
        .as_ref()
        .ok_or("queue not opened — connect first")?
        .clone();
    drop(queue_guard);
    let kind = DataKind::parse(&args.kind).ok_or_else(|| format!("unknown kind {}", args.kind))?;
    let format = OutputFormat::parse(&args.format)
        .ok_or_else(|| format!("unknown format {}", args.format))?;
    // Endpoints come in two shapes and the queue has to respect the
    // difference. One declares `date` and answers a single session, so
    // a window becomes one task per trading day. The other declares
    // `start_date`/`end_date` and answers the whole window in one call
    // — fanning *that* out produced N tasks each missing the arguments
    // the endpoint requires, so every one of them failed.
    let takes_single_date = thetadatadx::find(kind.endpoint())
        .is_some_and(|m| m.params.iter().any(|p| p.name == "date"));

    let units: Vec<(NaiveDate, Option<NaiveDate>)> = match (&args.date, &args.start, &args.end) {
        (Some(d), _, _) => vec![(parse_ymd(d)?, None)],
        (None, Some(s), Some(e)) => {
            let s = parse_ymd(s)?;
            let e = parse_ymd(e)?;
            if takes_single_date {
                let client_guard = state.client.read().await;
                let client = client_guard.as_ref().ok_or("client not connected")?.clone();
                drop(client_guard);
                client
                    .trading_days(&args.symbol, s, e)
                    .await
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .map(|d| (d, None))
                    .collect()
            } else {
                // The server caps a range call at 365 days, so a wider
                // window becomes several consecutive tasks rather than
                // one request it will reject.
                tdds_core::chunk_window(s, e)
                    .into_iter()
                    .map(|(from, to)| (from, Some(to)))
                    .collect()
            }
        }
        _ => return Err("pass date or start+end".into()),
    };
    // Several option endpoints refuse `expiration=*` outright — the
    // greeks series, option OHLC, and the two option list endpoints —
    // so a task carrying the wildcard could only ever fail. Resolve the
    // real expirations and fan out over them. Only the ones that were
    // still live during the requested window are worth asking for; an
    // expiration that had already passed has nothing to report.
    let requested_expiration = args.expiration.clone().unwrap_or_else(|| "*".into());
    let expirations: Vec<String> =
        if kind.rejects_expiration_wildcard() && is_wildcard(&requested_expiration) {
            let client_guard = state.client.read().await;
            let client = client_guard.as_ref().ok_or("client not connected")?.clone();
            drop(client_guard);
            let window_start = units.iter().map(|(d, _)| *d).min();
            let all = client
                .option_expirations(&args.symbol)
                .await
                .map_err(|e| e.to_string())?;
            let live: Vec<String> = all
                .into_iter()
                .filter(|e| window_start.is_none_or(|start| *e >= start))
                .map(|e| e.format("%Y%m%d").to_string())
                .collect();
            if live.is_empty() {
                return Err(format!(
                "{} needs a specific expiration and {} has none on or after the requested window",
                kind.as_str(),
                args.symbol
            ));
            }
            live
        } else {
            vec![requested_expiration]
        };

    let priority = args.priority.unwrap_or(0);
    let mut queued = 0usize;
    for (d, end) in &units {
        for expiration in &expirations {
            let spec = DataSpec {
                kind: kind.clone(),
                symbol: args.symbol.clone(),
                date: *d,
                end_date: *end,
                interval: args.interval.clone(),
                expiration: expiration.clone(),
                strike: args.strike.clone().unwrap_or_else(|| "*".into()),
                right: args.right.clone().unwrap_or_else(|| "both".into()),
                transforms: args.transforms.clone().unwrap_or_default(),
                extra: args.extra.clone().unwrap_or_default(),
            };
            queue
                .enqueue(spec, format, &cfg.output_dir, priority)
                .await
                .map_err(|e| e.to_string())?;
            queued += 1;
        }
    }
    Ok(queued)
}

/// `*` is the app's own default for "every expiration"; an empty field
/// means the same thing.
fn is_wildcard(v: &str) -> bool {
    v.is_empty() || v == "*"
}

/// How many rows a snapshot carries. The UI paginates against the
/// counts, which cover the whole table, so this bounds one payload
/// rather than the queue.
const SNAPSHOT_ROWS: i64 = 500;

/// How long a disk-footprint reading stays fresh.
const DISK_USAGE_TTL: std::time::Duration = std::time::Duration::from_secs(15);

/// On-disk footprint, walked at most once per [`DISK_USAGE_TTL`].
async fn disk_usage(state: &AppState, output_dir: &str) -> Result<DiskUsage, String> {
    {
        let cached = state.disk_usage.lock().await;
        if let Some((taken, usage)) = cached.as_ref() {
            if taken.elapsed() < DISK_USAGE_TTL {
                return Ok(*usage);
            }
        }
    }
    let cov = coverage::scan(&PathBuf::from(output_dir)).map_err(|e| e.to_string())?;
    let usage = DiskUsage {
        bytes: cov.iter().map(|c| c.bytes).sum(),
        files: cov.iter().map(|c| c.dates.len()).sum(),
    };
    *state.disk_usage.lock().await = Some((std::time::Instant::now(), usage));
    Ok(usage)
}

#[tauri::command]
pub async fn snapshot(state: State<'_, Arc<AppState>>) -> Result<QueueSnapshot, String> {
    let cfg = state.settings.read().await.clone();
    let queue_guard = state.queue.read().await;
    let queue = queue_guard.as_ref().ok_or("queue not opened")?.clone();
    drop(queue_guard);
    let counts = queue.counts().await.map_err(|e| e.to_string())?;
    let counts = counts
        .into_iter()
        .map(|(s, n)| (s.as_str().to_string(), n))
        .collect();
    let recent_tasks = queue
        .list(None, SNAPSHOT_ROWS)
        .await
        .map_err(|e| e.to_string())?;
    let recent: Vec<TaskView> = recent_tasks.into_iter().map(Into::into).collect();
    let usage = disk_usage(&state, &cfg.output_dir).await?;
    Ok(QueueSnapshot {
        counts,
        recent,
        bytes_on_disk: usage.bytes,
        files_on_disk: usage.files,
    })
}

/// Single-flight worker pool: a second click while a pool is in flight
/// is a no-op. Returns true if a new pool was started, false if one
/// was already running.
#[tauri::command]
pub async fn run_queue(
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<bool, String> {
    let mut handle_guard = state.worker_handle.lock().await;
    if let Some(h) = handle_guard.as_ref() {
        if !h.is_finished() {
            return Ok(false);
        }
    }
    let queue = {
        let g = state.queue.read().await;
        g.as_ref().ok_or("queue not opened")?.clone()
    };
    let client = {
        let g = state.client.read().await;
        g.as_ref().ok_or("client not connected")?.clone()
    };
    // The account-wide in-flight budget, read here rather than at
    // connect so a tier upgrade applies on the next run.
    let tiers = client.user_tiers();
    // Wire a progress channel: Pool emits Started/Done/Empty/Failed
    // events as workers tick; we forward each to the webview as a
    // tauri event so the UI can update without waiting for the 1.5 s
    // SQLite poll. Bounded mpsc keeps backpressure if the renderer
    // stalls.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ProgressEvent>(1024);
    let app_for_events = app.clone();
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            // Best-effort emit; if the window is gone we just stop.
            if app_for_events.emit("tdds:progress", &ev).is_err() {
                break;
            }
        }
    });
    let h = tokio::spawn(async move {
        let pool = Pool::new(client, queue, tiers).with_events(tx);
        // A pool error means the queue itself is unreachable; individual
        // task failures are recorded on their rows and never surface
        // here. Log it rather than dropping it on the floor.
        if let Err(e) = pool.run().await {
            tracing::error!(error = %e, "worker pool stopped early");
        }
    });
    *handle_guard = Some(h);
    Ok(true)
}

/// True iff a worker pool task is in flight.
#[tauri::command]
pub async fn worker_pool_active(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let g = state.worker_handle.lock().await;
    Ok(g.as_ref().is_some_and(|h| !h.is_finished()))
}

/// Take the live queue handle, or fail with a message the UI can show.
async fn queue_of(state: &AppState) -> Result<Queue, String> {
    let g = state.queue.read().await;
    Ok(g.as_ref().ok_or("queue not opened")?.clone())
}

/// Cancel pending or running tasks. A running task is allowed to finish
/// the request already in flight — the server is doing that work either
/// way — but its row flips immediately so the UI reflects the click.
/// Returns how many rows changed.
#[tauri::command]
pub async fn cancel_tasks(
    state: State<'_, Arc<AppState>>,
    ids: Vec<String>,
) -> Result<u64, String> {
    let queue = queue_of(&state).await?;
    queue.cancel_many(&ids).await.map_err(|e| e.to_string())
}

/// Put tasks back to pending, whatever their current status.
#[tauri::command]
pub async fn requeue_tasks(
    state: State<'_, Arc<AppState>>,
    ids: Vec<String>,
) -> Result<u64, String> {
    let queue = queue_of(&state).await?;
    queue.requeue_many(&ids).await.map_err(|e| e.to_string())
}

/// Delete tasks from the queue, whatever their status. A running task's
/// request finishes on the server either way; removing the row just
/// stops tracking it, so this needs no cancel first.
#[tauri::command]
pub async fn remove_tasks(
    state: State<'_, Arc<AppState>>,
    ids: Vec<String>,
) -> Result<u64, String> {
    let queue = queue_of(&state).await?;
    queue.remove_many(&ids).await.map_err(|e| e.to_string())
}

/// Delete every finished row, or every row in one finished status.
/// Acts on the whole queue, not just the page the UI has loaded.
#[tauri::command]
pub async fn clear_tasks(
    state: State<'_, Arc<AppState>>,
    status: Option<String>,
) -> Result<u64, String> {
    let queue = queue_of(&state).await?;
    let status = match status.as_deref() {
        None | Some("") | Some("all") => None,
        Some(s) => Some(TaskStatus::parse(s).ok_or_else(|| format!("unknown status {s}"))?),
    };
    queue
        .clear_finished(status)
        .await
        .map_err(|e| e.to_string())
}

/// Move pending tasks to the front of the queue. Returns how many moved;
/// a task a worker already claimed does not.
#[tauri::command]
pub async fn bump_tasks(
    state: State<'_, Arc<AppState>>,
    ids: Vec<String>,
) -> Result<usize, String> {
    let queue = queue_of(&state).await?;
    let mut moved = 0;
    for id in &ids {
        if queue.bump_priority(id).await.map_err(|e| e.to_string())? {
            moved += 1;
        }
    }
    Ok(moved)
}

/// Queue fresh copies of tasks. Useful for re-pulling a date whose file
/// was deleted, or re-running one that came back empty.
#[tauri::command]
pub async fn duplicate_tasks(
    state: State<'_, Arc<AppState>>,
    ids: Vec<String>,
) -> Result<usize, String> {
    let queue = queue_of(&state).await?;
    let mut made = 0;
    for id in &ids {
        queue.duplicate(id).await.map_err(|e| e.to_string())?;
        made += 1;
    }
    Ok(made)
}

#[tauri::command]
pub async fn requeue_failed(state: State<'_, Arc<AppState>>) -> Result<usize, String> {
    let queue_guard = state.queue.read().await;
    let queue = queue_guard.as_ref().ok_or("queue not opened")?.clone();
    drop(queue_guard);
    let failed = queue
        .list(Some(TaskStatus::Failed), 10_000)
        .await
        .map_err(|e| e.to_string())?;
    for t in &failed {
        queue.requeue(&t.id).await.map_err(|e| e.to_string())?;
    }
    Ok(failed.len())
}

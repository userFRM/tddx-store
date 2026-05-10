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
};

use crate::state::{parse_ymd, status_str, AppState, QueueSnapshot, TaskView};

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
    let dates: Vec<NaiveDate> = match (&args.date, &args.start, &args.end) {
        (Some(d), _, _) => vec![parse_ymd(d)?],
        (None, Some(s), Some(e)) => {
            let client_guard = state.client.read().await;
            let client = client_guard.as_ref().ok_or("client not connected")?.clone();
            drop(client_guard);
            let s = parse_ymd(s)?;
            let e = parse_ymd(e)?;
            client
                .trading_days(&args.symbol, s, e)
                .await
                .map_err(|e| e.to_string())?
        }
        _ => return Err("pass date or start+end".into()),
    };
    let priority = args.priority.unwrap_or(0);
    for d in &dates {
        let spec = DataSpec {
            kind,
            symbol: args.symbol.clone(),
            date: *d,
            interval: args.interval.clone(),
            expiration: args.expiration.clone().unwrap_or_else(|| "*".into()),
            strike: args.strike.clone().unwrap_or_else(|| "*".into()),
            right: args.right.clone().unwrap_or_else(|| "both".into()),
            transforms: args.transforms.clone().unwrap_or_default(),
        };
        queue
            .enqueue(spec, format, &cfg.output_dir, priority)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(dates.len())
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
        .map(|(s, n)| (status_str(s).to_string(), n))
        .collect();
    let recent_tasks = queue.list(None, 200).await.map_err(|e| e.to_string())?;
    let recent: Vec<TaskView> = recent_tasks.into_iter().map(Into::into).collect();
    let cov = coverage::scan(&PathBuf::from(&cfg.output_dir)).map_err(|e| e.to_string())?;
    let bytes_on_disk: u64 = cov.iter().map(|c| c.bytes).sum();
    let files_on_disk: usize = cov.iter().map(|c| c.dates.len()).sum();
    Ok(QueueSnapshot {
        counts,
        recent,
        bytes_on_disk,
        files_on_disk,
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
    // Per-class concurrency: 2^tier per ThetaData asset class.
    // Captured here (not at connect) so a tier upgrade picked up by
    // a fresh `tier_status` refresh is reflected on the next run.
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
        let _ = pool.run().await;
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

/// Cancel a pending or running task. Pending tasks die immediately; a
/// running task is allowed to finish its current gRPC call (no point
/// killing the request mid-flight — server is doing the work) but the
/// row is marked failed/cancelled so the UI updates.
#[tauri::command]
pub async fn cancel_task(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    let queue = {
        let g = state.queue.read().await;
        g.as_ref().ok_or("queue not opened")?.clone()
    };
    queue.cancel(&id).await.map_err(|e| e.to_string())
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

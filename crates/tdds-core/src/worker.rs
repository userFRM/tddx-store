//! Worker pool: pulls tasks from the queue, runs them through the
//! endpoint dispatcher, writes the requested format.
//!
//! One pool, sized by `UserTiers::in_flight_budget` — ThetaData's
//! limiter is account-wide, so there is one budget to spend and no
//! per-class lanes to split it into. Workers claim from the whole queue
//! in insertion order.
//!
//! The count is not a user setting; exposing it as one was a footgun
//! (over-provisioning buys server-side pacing, not throughput; under
//! leaves bandwidth idle).

use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::client::Client;
use crate::coverage::dataset_path;
use crate::format::write_batch;
use crate::progress::{Progress, ProgressEvent};
use crate::queue::{Queue, Task};
use crate::tier::UserTiers;

pub struct Pool {
    client: Client,
    queue: Queue,
    tiers: UserTiers,
    progress: Arc<Mutex<Progress>>,
    events_tx: Option<mpsc::Sender<ProgressEvent>>,
}

impl Pool {
    /// Build a pool sized by the user's per-class subscription tiers.
    /// `tiers` is typically `client.user_tiers()` taken at queue-run
    /// time (not connect time), so a tier upgrade mid-session
    /// propagates on the next run.
    pub fn new(client: Client, queue: Queue, tiers: UserTiers) -> Self {
        Self {
            client,
            queue,
            tiers,
            progress: Arc::new(Mutex::new(Progress::new())),
            events_tx: None,
        }
    }

    pub fn with_events(mut self, tx: mpsc::Sender<ProgressEvent>) -> Self {
        self.events_tx = Some(tx);
        self
    }

    pub fn progress(&self) -> Arc<Mutex<Progress>> {
        self.progress.clone()
    }

    /// Drain the queue. Returns once every worker has seen it empty.
    pub async fn run(&self) -> crate::Result<()> {
        let mut handles = Vec::new();
        for _ in 0..self.tiers.in_flight_budget() {
            let client = self.client.clone();
            let queue = self.queue.clone();
            let progress = self.progress.clone();
            let tx = self.events_tx.clone();
            handles.push(tokio::spawn(run_worker(client, queue, progress, tx)));
        }
        for h in handles {
            let _ = h.await;
        }
        Ok(())
    }
}

async fn run_worker(
    client: Client,
    queue: Queue,
    progress: Arc<Mutex<Progress>>,
    tx: Option<mpsc::Sender<ProgressEvent>>,
) {
    loop {
        let task = match queue.claim_next().await {
            Ok(Some(t)) => t,
            Ok(None) => break,
            Err(e) => {
                tracing::error!(?e, "claim failed");
                tokio::time::sleep(std::time::Duration::from_millis(
                    crate::config::CLAIM_RETRY_MS,
                ))
                .await;
                continue;
            }
        };
        {
            let mut p = progress.lock().await;
            p.running += 1;
        }
        if let Some(tx) = &tx {
            let _ = tx
                .send(ProgressEvent::Started {
                    task_id: task.id.clone(),
                })
                .await;
        }
        let heartbeat = spawn_heartbeat(queue.clone(), task.id.clone());
        // Isolate per-task panics so one buggy decode path
        // doesn't take a worker slot down silently. We wrap
        // the future in `AssertUnwindSafe + catch_unwind`
        // (via `FutureExt`) and translate a panic into a
        // typed error. Without this, a panic returns a
        // `JoinError` we'd ignore via `let _ = h.await`,
        // leaving the in-flight task pinned in `running`
        // forever and dropping our concurrency by one.
        use futures::FutureExt;
        let res = match std::panic::AssertUnwindSafe(run_one(&client, &task))
            .catch_unwind()
            .await
        {
            Ok(r) => r,
            Err(p) => {
                let msg = if let Some(s) = p.downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = p.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "worker panic (non-string payload)".to_string()
                };
                tracing::error!(task_id = %task.id, "worker panic: {msg}");
                Err(crate::Error::Other(format!("panic: {msg}")))
            }
        };
        heartbeat.stop().await;
        {
            let mut p = progress.lock().await;
            p.running = p.running.saturating_sub(1);
            match &res {
                Ok((rows, bytes)) => {
                    if *rows > 0 {
                        p.completed += 1;
                        p.rows_written += *rows as u64;
                        p.bytes_written += *bytes;
                    }
                }
                Err(_) => p.failed += 1,
            }
        }
        // mark_*() returns Ok(true) when it actually
        // updated a row, Ok(false) when the row was no
        // longer `running` (cancelled by the user via
        // queue::cancel) so the worker should NOT clobber
        // the cancelled state. SQL errors propagate via
        // `tracing::error!` because retry logic is
        // outside the per-row scope.
        match res {
            Ok((0, _ms)) => match queue.mark_empty(&task.id).await {
                Ok(true) => {
                    if let Some(tx) = &tx {
                        let _ = tx
                            .send(ProgressEvent::Empty {
                                task_id: task.id.clone(),
                                millis: 0,
                            })
                            .await;
                    }
                }
                Ok(false) => {
                    tracing::info!(
                        task_id = %task.id,
                        "task cancelled mid-flight; skipping mark_empty"
                    );
                }
                Err(e) => tracing::error!(?e, task_id = %task.id, "mark_empty"),
            },
            Ok((rows, bytes)) => match queue.mark_done(&task.id, rows as i64, bytes as i64).await {
                Ok(true) => {
                    if let Some(tx) = &tx {
                        let _ = tx
                            .send(ProgressEvent::Done {
                                task_id: task.id.clone(),
                                rows: rows as u64,
                                bytes,
                                millis: 0,
                            })
                            .await;
                    }
                }
                Ok(false) => {
                    tracing::info!(
                        task_id = %task.id,
                        "task cancelled mid-flight; skipping mark_done"
                    );
                }
                Err(e) => tracing::error!(?e, task_id = %task.id, "mark_done"),
            },
            Err(e) => {
                let msg = e.to_string();
                match queue.mark_failed(&task.id, &msg).await {
                    Ok(true) => {
                        if let Some(tx) = &tx {
                            let _ = tx
                                .send(ProgressEvent::Failed {
                                    task_id: task.id.clone(),
                                    error: msg,
                                    millis: 0,
                                })
                                .await;
                        }
                    }
                    Ok(false) => {
                        tracing::info!(
                            task_id = %task.id,
                            "task already terminal; not marking failed"
                        );
                    }
                    Err(e) => tracing::error!(?e, task_id = %task.id, "mark_failed"),
                }
            }
        }
    }
}

struct HeartbeatGuard {
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
    handle: tokio::task::JoinHandle<()>,
}

impl HeartbeatGuard {
    async fn stop(mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        let _ = self.handle.await;
    }
}

fn spawn_heartbeat(queue: Queue, task_id: String) -> HeartbeatGuard {
    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
    let handle = tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(crate::config::TASK_HEARTBEAT_INTERVAL_SECS);
        loop {
            tokio::select! {
                _ = &mut stop_rx => break,
                _ = tokio::time::sleep(interval) => {
                    match queue.heartbeat(&task_id).await {
                        Ok(true) => {}
                        Ok(false) => {
                            tracing::info!(task_id = %task_id, "heartbeat stopped; task no longer running or owned");
                            break;
                        }
                        Err(e) => tracing::error!(?e, task_id = %task_id, "heartbeat"),
                    }
                }
            }
        }
    });
    HeartbeatGuard {
        stop_tx: Some(stop_tx),
        handle,
    }
}

async fn run_one(client: &Client, task: &Task) -> crate::Result<(usize, u64)> {
    let out_dir = Path::new(&task.output_dir);
    let path = dataset_path(
        out_dir,
        &task.spec.kind,
        &task.spec.symbol,
        &task.spec.ymd(),
        task.format.extension(),
    );
    if path.exists() {
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        return Ok((0, bytes));
    }

    // One dispatch path for every kind: `DataSpec` lowers onto the same
    // registry spec the endpoint browser uses, so argument validation,
    // wire coercion, and Arrow column projection are identical whichever
    // surface queued the task.
    let batch = crate::registry::dispatch_to_arrow(client, &task.spec.to_endpoint_spec()).await?;
    let rows = match batch {
        Some(b) if b.num_rows() > 0 => {
            let b2 = task.spec.transforms.apply(&b)?;
            write_batch(&b2, &path, task.format)?;
            b.num_rows()
        }
        _ => 0,
    };
    let bytes = if rows > 0 {
        std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };
    Ok((rows, bytes))
}

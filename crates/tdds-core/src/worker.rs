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

/// Behaviour the user can choose, as opposed to limits the account
/// imposes.
#[derive(Debug, Clone, Copy)]
pub struct PoolOptions {
    /// Run fewer workers than the plan allows. The in-flight budget is
    /// account-wide, so someone running their own scripts against the
    /// same account needs to leave headroom for them. `None` uses the
    /// whole budget.
    pub max_concurrency: Option<usize>,
    /// Re-queue a failed windowed task as two halves rather than
    /// leaving it as one failed row.
    pub split_failed_windows: bool,
}

impl Default for PoolOptions {
    fn default() -> Self {
        Self {
            max_concurrency: None,
            split_failed_windows: true,
        }
    }
}

pub struct Pool {
    client: Client,
    queue: Queue,
    tiers: UserTiers,
    options: PoolOptions,
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
            options: PoolOptions::default(),
            progress: Arc::new(Mutex::new(Progress::new())),
            events_tx: None,
        }
    }

    pub fn with_options(mut self, options: PoolOptions) -> Self {
        self.options = options;
        self
    }

    /// How many workers this run will use: the plan's account-wide
    /// budget, lowered to the user's cap when they set one, and never
    /// fewer than one.
    pub fn worker_count(&self) -> usize {
        effective_workers(self.tiers.in_flight_budget(), self.options.max_concurrency)
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
        let split = self.options.split_failed_windows;
        for _ in 0..self.worker_count() {
            let client = self.client.clone();
            let queue = self.queue.clone();
            let progress = self.progress.clone();
            let tx = self.events_tx.clone();
            handles.push(tokio::spawn(run_worker(client, queue, progress, tx, split)));
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
    split_failed_windows: bool,
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
                Ok(Outcome::Written { rows, bytes }) => {
                    p.completed += 1;
                    p.rows_written += *rows as u64;
                    p.bytes_written += *bytes;
                }
                Ok(Outcome::AlreadyOnDisk { .. }) => p.completed += 1,
                Ok(Outcome::NoData) => {}
                Err(_) => p.failed += 1,
            }
        }
        // mark_*() returns Ok(true) when it actually updated a row,
        // Ok(false) when the row was no longer `running` (cancelled by
        // the user via queue::cancel), so the worker must not clobber
        // the cancelled state. SQL errors are logged here because retry
        // lives outside the per-row scope.
        match res {
            Ok(outcome) => match outcome.recorded() {
                None => match queue.mark_empty(&task.id).await {
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
                    Ok(false) => tracing::info!(
                        task_id = %task.id,
                        "task cancelled mid-flight; skipping mark_empty"
                    ),
                    Err(e) => tracing::error!(?e, task_id = %task.id, "mark_empty"),
                },
                Some((rows, bytes)) => {
                    match queue.mark_done(&task.id, rows as i64, bytes as i64).await {
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
                        Ok(false) => tracing::info!(
                            task_id = %task.id,
                            "task cancelled mid-flight; skipping mark_done"
                        ),
                        Err(e) => tracing::error!(?e, task_id = %task.id, "mark_done"),
                    }
                }
            },
            Err(e) => {
                let mut msg = e.to_string();
                // A task can now be a year wide, or a year wide per
                // expiration. Retrying all of that because it failed
                // near the end throws away everything that worked, so
                // re-queue it as two narrower windows instead: each
                // half is strictly smaller, so repeated failure
                // converges on the days that actually fail rather than
                // repeating the ones that do not.
                if split_failed_windows {
                    if let Some(n) = split_failed_window(&queue, &task, &msg).await {
                        msg = format!("{msg} (re-queued as {n} narrower windows)");
                    }
                }
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
                    Ok(false) => tracing::info!(
                        task_id = %task.id,
                        "task already terminal; not marking failed"
                    ),
                    Err(e) => tracing::error!(?e, task_id = %task.id, "mark_failed"),
                }
            }
        }
    }
}

/// The plan's budget, lowered to the user's cap, never below one. A cap
/// above the budget is not a way to exceed the plan — the server would
/// only throttle the extra workers — so it is clamped rather than
/// honoured.
fn effective_workers(budget: usize, cap: Option<usize>) -> usize {
    cap.map_or(budget, |c| c.min(budget)).max(1)
}

/// Re-queue a failed windowed task as two halves. Returns how many were
/// queued, or `None` when splitting does not apply.
///
/// Deliberately not attempted for a failure the request itself caused:
/// a malformed argument fails identically at any width, so splitting it
/// just doubles the number of rows saying the same thing.
async fn split_failed_window(queue: &Queue, task: &Task, error: &str) -> Option<usize> {
    if is_request_error(error) {
        return None;
    }
    let (first, second) = task.spec.split_window()?;
    let mut queued = 0;
    for spec in [first, second] {
        match queue
            .enqueue(spec, task.format, &task.output_dir, task.priority)
            .await
        {
            Ok(_) => queued += 1,
            Err(e) => {
                tracing::error!(?e, task_id = %task.id, "could not re-queue split window");
                return None;
            }
        }
    }
    Some(queued)
}

/// Whether the server rejected the shape of the request rather than
/// failing to deliver it. These are the app's bugs, not transient.
fn is_request_error(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("missing required arg")
        || m.contains("unknown endpoint")
        || m.contains("invalidargument")
        || m.contains("cannot specify")
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

/// What a single task did. Kept explicit because "wrote nothing"
/// and "the file was already there" are different facts, and collapsing
/// them into `rows == 0` reported every re-run of an existing file as
/// though the server had returned no data.
enum Outcome {
    /// Rows came back and were written.
    Written { rows: usize, bytes: u64 },
    /// The file was already on disk, so nothing was fetched.
    AlreadyOnDisk { bytes: u64 },
    /// The server returned no rows for this request.
    NoData,
}

impl Outcome {
    /// The `(rows, bytes)` to record against the task, or `None` when
    /// the request genuinely came back with nothing and the row should
    /// be marked empty rather than done.
    fn recorded(&self) -> Option<(usize, u64)> {
        match *self {
            Outcome::Written { rows, bytes } => Some((rows, bytes)),
            Outcome::AlreadyOnDisk { bytes } => Some((0, bytes)),
            Outcome::NoData => None,
        }
    }
}

async fn run_one(client: &Client, task: &Task) -> crate::Result<Outcome> {
    let out_dir = Path::new(&task.output_dir);
    let path = dataset_path(out_dir, &task.spec, task.format.extension());
    if path.exists() {
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        return Ok(Outcome::AlreadyOnDisk { bytes });
    }

    // One dispatch path for every kind: `DataSpec` lowers onto the same
    // registry spec the endpoint browser uses, so argument validation,
    // wire coercion, and Arrow column projection are identical whichever
    // surface queued the task.
    let batch = crate::registry::dispatch_to_arrow(client, &task.spec.to_endpoint_spec()).await?;
    let Some(batch) = batch.filter(|b| b.num_rows() > 0) else {
        return Ok(Outcome::NoData);
    };
    let rows = batch.num_rows();
    let transformed = task.spec.transforms.apply(&batch)?;
    write_batch(&transformed, &path, task.format)?;
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(Outcome::Written { rows, bytes })
}

#[cfg(test)]
mod pool_tests {
    use super::effective_workers;

    #[test]
    fn no_cap_uses_the_whole_budget() {
        assert_eq!(effective_workers(8, None), 8);
    }

    #[test]
    fn a_cap_below_the_budget_leaves_headroom() {
        assert_eq!(effective_workers(8, Some(3)), 3);
    }

    #[test]
    fn a_cap_above_the_budget_cannot_exceed_the_plan() {
        assert_eq!(effective_workers(8, Some(64)), 8);
    }

    #[test]
    fn there_is_always_at_least_one_worker() {
        assert_eq!(effective_workers(8, Some(0)), 1);
        assert_eq!(effective_workers(0, None), 1);
    }
}

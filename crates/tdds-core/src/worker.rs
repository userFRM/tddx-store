//! Worker pool: pulls tasks from the queue, hits thetadatadx, writes the
//! requested format.
//!
//! Concurrency is sliced per **asset class**, not per process. ThetaData's
//! FPSS server applies an independent in-flight cap of `2^tier` to each
//! Nexus pool (stock, option, index, rate). A user holding Pro Options +
//! Standard Stocks can run 8 option workers and 4 stock workers
//! simultaneously without the server queueing or 429ing either lane.
//!
//! We mirror that on the client by spawning one logical sub-pool per
//! asset class, sized from `UserTiers`. Each sub-pool calls
//! `Queue::claim_next_by_class(class)`, which restricts the atomic claim
//! to that class's `kind` set — so a stock worker can never accidentally
//! pick up an option task and double-book the option budget.
//!
//! The `workers` count is no longer a user setting; exposing it as one
//! was a footgun (over-provisioning → 429s, under → idle bandwidth).

use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::client::Client;
use crate::coverage::dataset_path;
use crate::format::{write_batch, TicksArrowExt};
use crate::progress::{Progress, ProgressEvent};
use crate::queue::{Queue, Task};
use crate::spec::DataKind;
use crate::tier::{AssetClass, UserTiers};

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

    /// Drain the queue. Spawns per-class worker tasks; each pulls only
    /// from its class's kind set. Returns when every class is drained.
    pub async fn run(&self) -> crate::Result<()> {
        // Stock + Option are the only classes with shipping kinds.
        // Index/Rate are reserved for future endpoints — `claim_next_by_class`
        // short-circuits them so we don't burn a SQL hit per spin.
        let classes = [AssetClass::Stock, AssetClass::Option];
        let mut handles = Vec::new();
        for class in classes {
            let n = self.tiers.workers_for(class);
            if n == 0 {
                continue;
            }
            for _ in 0..n {
                let client = self.client.clone();
                let queue = self.queue.clone();
                let progress = self.progress.clone();
                let tx = self.events_tx.clone();
                handles.push(tokio::spawn(run_worker(class, client, queue, progress, tx)));
            }
        }
        for h in handles {
            let _ = h.await;
        }
        Ok(())
    }
}

async fn run_worker(
    class: AssetClass,
    client: Client,
    queue: Queue,
    progress: Arc<Mutex<Progress>>,
    tx: Option<mpsc::Sender<ProgressEvent>>,
) {
    loop {
        let task = match queue.claim_next_by_class(class).await {
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
        task.spec.kind,
        &task.spec.symbol,
        &task.spec.ymd(),
        task.format.extension(),
    );
    if path.exists() {
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        return Ok((0, bytes));
    }

    let raw = client.raw();
    let symbol = &task.spec.symbol;
    let ds = task.spec.ymd();
    let interval = task.spec.interval.as_deref().unwrap_or("0");
    let exp = &task.spec.expiration;
    let strike = &task.spec.strike;
    let right = &task.spec.right;
    let fmt = task.format;

    // Each tick type implements `TicksArrowExt` on its slice. We resolve
    // the trait at the call site (where T is concrete) and hand the
    // resulting RecordBatch to the format-agnostic writer.
    let rows = match task.spec.kind {
        DataKind::StockTrade => {
            let v = raw.stock_history_trade(symbol, &ds).await?;
            if v.is_empty() {
                0
            } else {
                let b = v.as_slice().to_arrow()?;
                let b2 = task.spec.transforms.apply(&b)?;
                write_batch(&b2, &path, fmt)?;
                v.len()
            }
        }
        DataKind::StockQuote => {
            let v = raw
                .stock_history_quote(symbol, &ds)
                .interval(interval)
                .await?;
            if v.is_empty() {
                0
            } else {
                let b = v.as_slice().to_arrow()?;
                let b2 = task.spec.transforms.apply(&b)?;
                write_batch(&b2, &path, fmt)?;
                v.len()
            }
        }
        DataKind::StockTradeQuote => {
            let v = raw.stock_history_trade_quote(symbol, &ds).await?;
            if v.is_empty() {
                0
            } else {
                let b = v.as_slice().to_arrow()?;
                let b2 = task.spec.transforms.apply(&b)?;
                write_batch(&b2, &path, fmt)?;
                v.len()
            }
        }
        DataKind::OptionTrade => {
            let v = raw
                .option_history_trade(symbol, exp, &ds)
                .strike(strike)
                .right(right)
                .await?;
            if v.is_empty() {
                0
            } else {
                let b = v.as_slice().to_arrow()?;
                let b2 = task.spec.transforms.apply(&b)?;
                write_batch(&b2, &path, fmt)?;
                v.len()
            }
        }
        DataKind::OptionQuote => {
            let v = raw
                .option_history_quote(symbol, exp, &ds)
                .strike(strike)
                .right(right)
                .interval(interval)
                .await?;
            if v.is_empty() {
                0
            } else {
                let b = v.as_slice().to_arrow()?;
                let b2 = task.spec.transforms.apply(&b)?;
                write_batch(&b2, &path, fmt)?;
                v.len()
            }
        }
        DataKind::OptionTradeQuote => {
            let v = raw
                .option_history_trade_quote(symbol, exp, &ds)
                .strike(strike)
                .right(right)
                .await?;
            if v.is_empty() {
                0
            } else {
                let b = v.as_slice().to_arrow()?;
                let b2 = task.spec.transforms.apply(&b)?;
                write_batch(&b2, &path, fmt)?;
                v.len()
            }
        }
        DataKind::OptionOpenInterest => {
            let v = raw
                .option_history_open_interest(symbol, exp, &ds)
                .strike(strike)
                .right(right)
                .await?;
            if v.is_empty() {
                0
            } else {
                let b = v.as_slice().to_arrow()?;
                let b2 = task.spec.transforms.apply(&b)?;
                write_batch(&b2, &path, fmt)?;
                v.len()
            }
        }
    };
    let bytes = if rows > 0 {
        std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };
    Ok((rows, bytes))
}

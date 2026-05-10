//! ThetaData connection lifecycle.
//!
//! `connect` opens the SQLite queue + builds a `tdds_core::Client` and
//! commits both into `AppState` atomically (caller never observes a
//! half-initialised state). `login` is the email/password variant the
//! GUI uses — it persists the credentials in-memory and delegates.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use tauri::State;
use tdds_core::{schedule, Client, Queue};

use crate::state::AppState;

#[tauri::command]
pub async fn connect(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    // Snapshot settings under a short read lock — never held across an
    // await on the network or filesystem.
    let cfg = state.settings.read().await.clone();
    std::fs::create_dir_all(PathBuf::from(&cfg.output_dir).as_path()).map_err(|e| e.to_string())?;
    if let Some(parent) = PathBuf::from(&cfg.db_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // Build BOTH the queue and the client into locals first, only
    // committing them to AppState after both succeed. Avoids the
    // half-initialised state where `queue` is set but `client` failed,
    // which leaves the UI showing "queue ready" while every network
    // operation 500s.
    let queue = Queue::open(&PathBuf::from(&cfg.db_path))
        .await
        .map_err(|e| e.to_string())?;
    let client = if !cfg.email.is_empty() && !cfg.password.is_empty() {
        Client::connect_with_credentials(&cfg.email, &cfg.password)
            .await
            .map_err(|e| e.to_string())?
    } else {
        Client::connect(Some(&PathBuf::from(&cfg.creds_path)))
            .await
            .map_err(|e| e.to_string())?
    };
    // Reap any tasks left in `running` state by a previous unclean
    // shutdown (the rust runtime aborted mid-task; they got stuck in
    // SQLite). New pool runs will not pick them up again unless reset
    // to `pending`.
    queue
        .reset_running_to_pending()
        .await
        .map_err(|e| e.to_string())?;
    // Ensure the schedules table exists before any UI tab tries to
    // list / insert rows. `Queue::open` only creates the queue table;
    // the scheduler module owns its own DDL and is otherwise lazy on
    // the first `schedule_create` call. Without this, opening the
    // Schedules tab on a fresh install fires `no such table: schedules`.
    schedule::create_table(queue.pool())
        .await
        .map_err(|e| e.to_string())?;
    // Atomic commit: take both write locks together so a poller sees
    // either both fields populated or neither.
    {
        let mut q_w = state.queue.write().await;
        let mut c_w = state.client.write().await;
        *q_w = Some(queue);
        *c_w = Some(client);
    }
    Ok("connected".into())
}

#[derive(Deserialize)]
pub struct LoginArgs {
    pub email: String,
    pub password: String,
}

/// Direct email/password login. Stores credentials in memory and
/// delegates to `connect`. Replaces the creds-file flow for the GUI.
#[tauri::command]
pub async fn login(state: State<'_, Arc<AppState>>, args: LoginArgs) -> Result<String, String> {
    {
        let mut s = state.settings.write().await;
        s.email = args.email.clone();
        s.password = args.password.clone();
    }
    connect(state).await
}

/// Tear down the live session: clears the in-memory `Client`, wipes
/// credentials, and best-effort cancels any in-flight worker pool so
/// the next login doesn't inherit half-finished tasks. The queue
/// database stays open — task history is persistent, not session
/// state — but no further endpoint calls will succeed until the user
/// signs back in.
#[tauri::command]
pub async fn logout(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    // Stop the worker pool first so it can't observe a half-dropped
    // client mid-`claim_next_by_class`. Aborting is best-effort: if a
    // task is mid-network-request the abort fires after the in-flight
    // future yields. We don't await the JoinHandle past abort because
    // a hung future shouldn't block sign-out.
    {
        let mut h = state.worker_handle.lock().await;
        if let Some(handle) = h.take() {
            handle.abort();
        }
    }
    {
        let mut c = state.client.write().await;
        *c = None;
    }
    {
        let mut s = state.settings.write().await;
        s.email.clear();
        s.password.clear();
    }
    Ok(())
}

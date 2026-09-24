//! ThetaData connection lifecycle.
//!
//! `connect` opens the SQLite queue + builds a `tdds_core::Client` and
//! commits both into `AppState` atomically (caller never observes a
//! half-initialised state). `login` takes whichever credential the user
//! signed in with, holds it in memory for the session, and delegates.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use tauri::State;
use tdds_core::{schedule, Client, Queue};

use crate::state::AppState;

/// How long sign-in may take before the UI is told it failed. A normal
/// sign-in takes about two seconds.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

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
    // An API key entered this session wins, then email + password, then
    // whatever `THETADATA_API_KEY` or the creds file supplies.
    // The SDK's sign-in has no deadline of its own: a stalled auth or
    // channel handshake left the sign-in screen spinning forever with
    // nothing in the log after "authenticating". Bound it and say so.
    let connecting = async {
        if !cfg.api_key.is_empty() {
            let email = (!cfg.email.is_empty()).then_some(cfg.email.as_str());
            Client::connect_with_api_key(&cfg.api_key, email).await
        } else if !cfg.email.is_empty() && !cfg.password.is_empty() {
            Client::connect_with_credentials(&cfg.email, &cfg.password).await
        } else {
            Client::connect(Some(&PathBuf::from(&cfg.creds_path))).await
        }
    };
    let client = tokio::time::timeout(CONNECT_TIMEOUT, connecting)
        .await
        .map_err(|_| {
            format!(
                "ThetaData didn't answer within {} s. Check the connection and try again.",
                CONNECT_TIMEOUT.as_secs()
            )
        })?
        .map_err(|e| e.to_string())?;
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

/// How the user signed in. Tagged rather than a bag of optional fields
/// so "an API key and a blank password" is not representable.
#[derive(Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum LoginArgs {
    Password {
        email: String,
        password: String,
    },
    /// `email` is optional: market data needs only the key, flat files
    /// need the account email alongside it.
    ApiKey {
        api_key: String,
        #[serde(default)]
        email: Option<String>,
    },
}

/// Sign in and connect. Holds the credential in memory for the session
/// (never in the serialized settings) and delegates to `connect`.
#[tauri::command]
pub async fn login(state: State<'_, Arc<AppState>>, args: LoginArgs) -> Result<String, String> {
    {
        let mut s = state.settings.write().await;
        // Clear the other method's fields so a second sign-in cannot
        // connect with a credential the user thinks they replaced.
        s.email.clear();
        s.password.clear();
        s.api_key.clear();
        match args {
            LoginArgs::Password { email, password } => {
                s.email = email;
                s.password = password;
            }
            LoginArgs::ApiKey { api_key, email } => {
                s.api_key = api_key;
                s.email = email.unwrap_or_default();
            }
        }
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
    // client mid-`claim_next`. Aborting is best-effort: if a
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
        s.api_key.clear();
    }
    Ok(())
}

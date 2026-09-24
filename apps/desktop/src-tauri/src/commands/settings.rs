//! Settings, held in `AppState` and persisted next to the vault.
//!
//! The paths here decide where the queue database and every downloaded
//! file live, so they have to survive a restart: pointing the output at
//! an external drive, seeing "Saved", and finding the default path back
//! after relaunch is worse than not offering the control. Only the paths
//! are written — the credential fields are `skip_serializing` and stay
//! in memory, where the vault owns persistence.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;

use crate::secrets;
use crate::state::{AppState, Settings};

const FILE: &str = "settings.json";

fn settings_path() -> PathBuf {
    secrets::data_dir().join(FILE)
}

/// Read persisted settings, or `None` when there are none yet or the
/// file is unreadable. A corrupt file is not fatal: the setup hook seeds
/// defaults over the top, which is better than refusing to launch.
pub fn load_persisted() -> Option<Settings> {
    let raw = std::fs::read_to_string(settings_path()).ok()?;
    match serde_json::from_str(&raw) {
        Ok(settings) => Some(settings),
        Err(e) => {
            tracing::warn!(error = %e, "settings file is unreadable; using defaults");
            None
        }
    }
}

/// Write settings to disk. Goes through a temporary file and a rename so
/// an interrupted write cannot leave a half-written file that fails to
/// parse on the next launch.
fn persist(settings: &Settings) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("settings dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| format!("settings: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("write settings: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("commit settings: {e}"))
}

#[tauri::command]
pub async fn settings_get(state: State<'_, Arc<AppState>>) -> Result<Settings, String> {
    Ok(state.settings.read().await.clone())
}

#[tauri::command]
pub async fn settings_set(
    state: State<'_, Arc<AppState>>,
    settings: Settings,
) -> Result<(), String> {
    // `db_path` is not just a stored string: `connect` opens the queue
    // from it once and `AppState` holds that handle for the session.
    // Saving a new path used to update the setting and leave the live
    // queue on the old file, so the user saw "Saved" while every
    // subsequent enqueue still wrote to the database they had just
    // moved away from.
    let (previous_db, previous_out) = {
        let s = state.settings.read().await;
        (s.db_path.clone(), s.output_dir.clone())
    };
    let db_changed = previous_db != settings.db_path && !settings.db_path.is_empty();

    if db_changed && workers_running(&state).await {
        return Err("Downloads are running. Stop them before moving the queue database.".into());
    }

    persist(&settings)?;

    if db_changed {
        // Open the replacement before dropping the old handle, so a
        // path that cannot be opened leaves the session working.
        let path = PathBuf::from(&settings.db_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("queue dir: {e}"))?;
        }
        let opened = tdds_core::Queue::open(&path)
            .await
            .map_err(|e| format!("open queue at {}: {e}", settings.db_path))?;
        tdds_core::schedule::create_table(opened.pool())
            .await
            .map_err(|e| format!("schedules table: {e}"))?;
        let mut guard = state.queue.write().await;
        if guard.is_some() {
            *guard = Some(opened);
        }
    }

    // The frontend never holds the credential fields — they are
    // `skip_serializing`, so they never reach it — which means the
    // struct it sends back always has them empty. Replacing wholesale
    // would sign the session out of its own credentials on every
    // "Save", and the next reconnect would fail. Keep what is in
    // memory; the vault owns those.
    let mut current = state.settings.write().await;
    let mut settings = settings;
    settings.email = std::mem::take(&mut current.email);
    settings.password = std::mem::take(&mut current.password);
    settings.api_key = std::mem::take(&mut current.api_key);
    let new_out = settings.output_dir.clone();
    *current = settings;
    drop(current);
    // The Library follows the directory it shows.
    if new_out != previous_out {
        crate::events::watch_output_dir(state.inner(), &new_out);
        *state.disk_usage.lock().await = None;
        state.notify(crate::events::LIBRARY_CHANGED);
    }
    Ok(())
}

async fn workers_running(state: &AppState) -> bool {
    let guard = state.worker_handle.lock().await;
    guard.as_ref().is_some_and(|h| !h.is_finished())
}

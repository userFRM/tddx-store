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
    persist(&settings)?;
    *state.settings.write().await = settings;
    Ok(())
}

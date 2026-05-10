//! Settings CRUD. Backed by `RwLock<Settings>` in `AppState`; defaults
//! are seeded from `app.path().app_data_dir()` in the Tauri setup hook.

use std::sync::Arc;

use tauri::State;

use crate::state::{AppState, Settings};

#[tauri::command]
pub async fn settings_get(state: State<'_, Arc<AppState>>) -> Result<Settings, String> {
    Ok(state.settings.read().await.clone())
}

#[tauri::command]
pub async fn settings_set(
    state: State<'_, Arc<AppState>>,
    settings: Settings,
) -> Result<(), String> {
    *state.settings.write().await = settings;
    Ok(())
}

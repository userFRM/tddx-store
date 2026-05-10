//! Scheduled-download CRUD. The schedule table lives alongside the
//! task queue in the same SQLite file (`schedules`), with a separate
//! ticker (in `tdds-core` or `tdds-cli schedule run`) firing rows.

use std::sync::Arc;

use serde::Deserialize;
use tauri::State;
use tdds_core::{format::OutputFormat, schedule, DataKind, Schedule};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct ScheduleCreateArgs {
    pub name: String,
    pub kind: String,
    pub symbol: String,
    pub format: String,
    pub cron_kind: String, // "daily" | "weekdays" | "weekly:mon" …
    pub at_time: String,   // "HH:MM"
}

#[tauri::command]
pub async fn schedule_list(state: State<'_, Arc<AppState>>) -> Result<Vec<Schedule>, String> {
    let queue_guard = state.queue.read().await;
    let queue = queue_guard.as_ref().ok_or("queue not opened")?.clone();
    drop(queue_guard);
    schedule::list(queue.pool())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn schedule_create(
    state: State<'_, Arc<AppState>>,
    args: ScheduleCreateArgs,
) -> Result<Schedule, String> {
    let queue_guard = state.queue.read().await;
    let queue = queue_guard.as_ref().ok_or("queue not opened")?.clone();
    drop(queue_guard);
    schedule::create_table(queue.pool())
        .await
        .map_err(|e| e.to_string())?;
    let kind = DataKind::parse(&args.kind).ok_or_else(|| format!("unknown kind {}", args.kind))?;
    let format = OutputFormat::parse(&args.format)
        .ok_or_else(|| format!("unknown format {}", args.format))?;
    let s = Schedule {
        id: uuid::Uuid::new_v4().to_string(),
        name: args.name,
        kind,
        symbol: args.symbol,
        format,
        cron_kind: args.cron_kind,
        at_time: args.at_time,
        last_fired_at: None,
        paused: false,
        created_at: chrono::Utc::now().timestamp(),
    };
    schedule::insert(queue.pool(), &s)
        .await
        .map_err(|e| e.to_string())?;
    Ok(s)
}

#[tauri::command]
pub async fn schedule_delete(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    let queue_guard = state.queue.read().await;
    let queue = queue_guard.as_ref().ok_or("queue not opened")?.clone();
    drop(queue_guard);
    schedule::delete(queue.pool(), &id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn schedule_set_paused(
    state: State<'_, Arc<AppState>>,
    id: String,
    paused: bool,
) -> Result<(), String> {
    let queue_guard = state.queue.read().await;
    let queue = queue_guard.as_ref().ok_or("queue not opened")?.clone();
    drop(queue_guard);
    schedule::set_paused(queue.pool(), &id, paused)
        .await
        .map_err(|e| e.to_string())
}

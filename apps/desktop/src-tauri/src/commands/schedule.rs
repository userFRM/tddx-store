//! Scheduled-download CRUD, plus the ticker that fires the rows.
//!
//! The schedule table lives alongside the task queue in the same SQLite
//! file. [`spawn_ticker`] polls it once a minute and enqueues the due
//! rows; without it the table would be a list the app writes and never
//! reads, which is what it was.

use std::sync::Arc;

use serde::Deserialize;
use tauri::State;
use tdds_core::{format::OutputFormat, schedule, DataKind, DataSpec, Schedule};

use crate::events;
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
    state.notify(events::SCHEDULES_CHANGED);
    Ok(s)
}

#[tauri::command]
pub async fn schedule_delete(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    let queue_guard = state.queue.read().await;
    let queue = queue_guard.as_ref().ok_or("queue not opened")?.clone();
    drop(queue_guard);
    let r = schedule::delete(queue.pool(), &id)
        .await
        .map_err(|e| e.to_string());
    state.notify_ok(events::SCHEDULES_CHANGED, r)
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
    let r = schedule::set_paused(queue.pool(), &id, paused)
        .await
        .map_err(|e| e.to_string());
    state.notify_ok(events::SCHEDULES_CHANGED, r)
}

/// Poll the schedule table once a minute and enqueue whatever is due.
///
/// Runs for the life of the process, started from the Tauri setup hook.
/// Before the user connects there is no queue, so the tick is a cheap
/// no-op rather than an error.
pub fn spawn_ticker(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(SCHEDULE_TICK_SECS));
        loop {
            ticker.tick().await;
            if let Err(e) = tick(&state).await {
                tracing::warn!(error = %e, "schedule tick failed");
            }
        }
    });
}

/// How often the ticker wakes. A schedule's resolution is one minute,
/// so polling faster only costs a SQL round-trip.
const SCHEDULE_TICK_SECS: u64 = 60;

async fn tick(state: &AppState) -> Result<(), String> {
    let queue = {
        let guard = state.queue.read().await;
        match guard.as_ref() {
            Some(q) => q.clone(),
            None => return Ok(()),
        }
    };
    let output_dir = state.settings.read().await.output_dir.clone();
    if output_dir.is_empty() {
        return Ok(());
    }

    let now = chrono::Utc::now();
    let rows = schedule::due(queue.pool(), now)
        .await
        .map_err(|e| e.to_string())?;

    let mut fired = false;
    for row in rows {
        // Which session is available is an Eastern-time question, and
        // separate from the local-clock time the user picked to fire
        // at. The queue skips a file that is already on disk, so a
        // re-fire costs nothing.
        let Some(date) = schedule::last_available_session(now) else {
            continue;
        };
        let spec = DataSpec {
            kind: row.kind.clone(),
            symbol: row.symbol.clone(),
            date,
            interval: None,
            expiration: "*".into(),
            strike: "*".into(),
            right: "both".into(),
            end_date: None,
            transforms: Default::default(),
            extra: Default::default(),
        };
        // One download per fire, so a scheduled run shows up in the
        // Downloads view like anything queued by hand.
        let batch = format!("schedule:{}:{}", row.id, date.format("%Y%m%d"));
        match queue
            .enqueue_in_batch(spec, row.format, &output_dir, 0, Some(&batch))
            .await
        {
            Ok(_) => {
                fired = true;
                if let Err(e) = schedule::mark_fired(queue.pool(), &row.id, now.timestamp()).await {
                    tracing::error!(error = %e, schedule = %row.id, "mark_fired failed");
                } else {
                    tracing::info!(
                        schedule = %row.id, name = %row.name, symbol = %row.symbol,
                        date = %date, "schedule fired"
                    );
                }
            }
            // Leave `last_fired_at` alone so the next tick retries rather
            // than silently skipping a day.
            Err(e) => tracing::error!(error = %e, schedule = %row.id, "schedule enqueue failed"),
        }
    }
    if fired {
        state.notify(events::QUEUE_CHANGED);
        state.notify(events::SCHEDULES_CHANGED);
    }
    Ok(())
}

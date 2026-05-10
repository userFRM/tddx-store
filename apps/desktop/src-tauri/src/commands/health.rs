//! Read-only telemetry surface for the Health panel: pool size,
//! in-flight workers, queue counts, on-disk totals, uptime, SDK
//! versions. Cheap to compute (one SQLite COUNT() and one disk scan).
//!
//! Versions are sourced via Tauri's `package_info()` (this binary's
//! Cargo manifest) and the runtime-resolved Cargo.lock entries from
//! `build.rs`, so the Health panel reflects exactly what's bundled.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use tdds_core::coverage;
use tdds_core::tier::{AssetClass, UserTiers};

use crate::state::{status_str, AppState};

/// Process-wide boot timestamp for uptime reporting. Set by `lib::run`
/// in the Tauri setup hook so the value is anchored to the moment the
/// app entered `tauri::Builder::run`.
pub static APP_BOOT_TS: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

#[derive(Serialize)]
pub struct HealthSnapshot {
    /// Total in-flight budget across every class (sum of class pools).
    /// Mirrors the server's `2^tier_class` cap summed across classes.
    pub pool_size: u32,
    /// Per-class worker slots: `stock`, `option`, `index`, `rate`.
    /// Each class's value is `Tier::workers()` for that class's user
    /// tier; 0 when no kinds in that class ship yet (index/rate).
    pub pool_per_class: PoolPerClass,
    pub workers_in_flight: usize,
    pub pool_active: bool,
    pub task_counts: std::collections::BTreeMap<String, i64>,
    pub total_files_on_disk: usize,
    pub total_bytes_on_disk: u64,
    pub uptime_secs: u64,
    pub desktop_version: String,
    pub thetadatadx_version: String,
    pub tdbe_version: String,
}

#[derive(Serialize)]
pub struct PoolPerClass {
    pub stock: u32,
    pub option: u32,
    pub index: u32,
    pub rate: u32,
}

impl PoolPerClass {
    fn from_tiers(t: &UserTiers) -> Self {
        Self {
            stock: t.workers_for(AssetClass::Stock) as u32,
            option: t.workers_for(AssetClass::Option) as u32,
            index: t.workers_for(AssetClass::Index) as u32,
            rate: t.workers_for(AssetClass::Rate) as u32,
        }
    }
}

#[tauri::command]
pub async fn health(
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<HealthSnapshot, String> {
    let queue_opt = state.queue.read().await.as_ref().cloned();
    let pool_active = state
        .worker_handle
        .lock()
        .await
        .as_ref()
        .is_some_and(|h| !h.is_finished());
    let cfg = state.settings.read().await.clone();
    let mut task_counts = std::collections::BTreeMap::new();
    if let Some(queue) = queue_opt.as_ref() {
        if let Ok(counts) = queue.counts().await {
            for (s, n) in counts {
                task_counts.insert(status_str(s).to_string(), n);
            }
        }
    }
    let cov = coverage::scan(&PathBuf::from(&cfg.output_dir)).map_err(|e| e.to_string())?;
    let total_bytes_on_disk = cov.iter().map(|c| c.bytes).sum();
    let total_files_on_disk = cov.iter().map(|c| c.dates.len()).sum();
    let uptime_secs = APP_BOOT_TS
        .get()
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);
    let desktop_version = app.package_info().version.to_string();
    // Pool sizing is now derived from the live ThetaData subscription —
    // not a user setting. Pre-connect (no client) we report a zeroed
    // breakdown so the UI doesn't lie about server-allowed concurrency.
    let tiers = state
        .client
        .read()
        .await
        .as_ref()
        .map(|c| c.user_tiers())
        .unwrap_or_default();
    let pool_per_class = PoolPerClass::from_tiers(&tiers);
    let pool_size =
        pool_per_class.stock + pool_per_class.option + pool_per_class.index + pool_per_class.rate;
    Ok(HealthSnapshot {
        pool_size,
        pool_per_class,
        workers_in_flight: task_counts.get("running").copied().unwrap_or(0) as usize,
        pool_active,
        task_counts,
        total_files_on_disk,
        total_bytes_on_disk,
        uptime_secs,
        desktop_version,
        thetadatadx_version: env!("TDDS_THETADATADX_VERSION").to_string(),
        tdbe_version: env!("TDDS_TDBE_VERSION").to_string(),
    })
}

#[tauri::command]
pub async fn sdk_version(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "desktop":     app.package_info().version.to_string(),
        "thetadatadx": env!("TDDS_THETADATADX_VERSION"),
        "tdbe":        env!("TDDS_TDBE_VERSION"),
    }))
}

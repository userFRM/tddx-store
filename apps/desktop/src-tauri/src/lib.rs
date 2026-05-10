//! TdDx Store — desktop app entry.
//!
//! This file is intentionally small: it wires the Tauri builder, all
//! plugins, the setup hook (paths + Settings defaults via
//! `app.path().app_data_dir()`), and the invoke handler. Every command
//! body lives in `commands/<domain>.rs`. See `state.rs` for the shared
//! `AppState` + serializable types.

use std::sync::Arc;

use tauri::Manager;

mod commands;
mod secrets;
mod state;

use commands::{
    connection, coverage as coverage_cmd, endpoints, flatfiles, health, index_presets, preview,
    queue, schedule, settings, tier, vault,
};
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::try_init().ok();
    health::APP_BOOT_TS.set(std::time::Instant::now()).ok();

    tauri::Builder::default()
        // tauri-plugin-single-instance: a second `tdds-desktop` invocation
        // pushes its argv to the existing process and exits, focusing the
        // already-running window. Idiomatic for desktop apps that touch
        // a SQLite queue + Stronghold vault — running twice would corrupt
        // both. Must register first so the second-launch handshake fires
        // before any other plugin initializes.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
                let _ = w.unminimize();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        // tauri-plugin-sql with sqlite feature: gives the FE typed SQL
        // access to the queue.db (read-only views, ad-hoc reporting)
        // alongside the typed Tauri commands. The Rust backend still
        // owns writes via sqlx — plugin is for FE convenience.
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        // tauri-plugin-window-state: persists window position + size +
        // maximize state to the OS-correct config dir, restored on the
        // next launch. Zero-cost UX win.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        // tauri-plugin-updater: in-app update checks against the
        // releases JSON manifest hosted on GitHub Releases. Inactive
        // until a signing key is configured (`tauri signer generate`)
        // and an `updater.endpoints` URL is added to tauri.conf.json;
        // registering it now is just plumbing so a future signed
        // release can wire updates without an upgrade hop.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_stronghold::Builder::new(secrets::vault_hasher_fn()).build())
        .manage(Arc::new(AppState::default()))
        .invoke_handler(tauri::generate_handler![
            settings::settings_get,
            settings::settings_set,
            connection::connect,
            connection::login,
            connection::logout,
            queue::enqueue,
            queue::snapshot,
            queue::run_queue,
            queue::worker_pool_active,
            queue::cancel_task,
            queue::requeue_failed,
            coverage_cmd::coverage_report,
            coverage_cmd::duckdb_command,
            endpoints::endpoints_list,
            endpoints::endpoints_get,
            endpoints::endpoint_invoke,
            endpoints::list_query,
            endpoints::dataset_catalogue,
            endpoints::dataset_metadata,
            flatfiles::flatfile_download,
            index_presets::index_presets,
            index_presets::index_constituents,
            preview::parquet_preview,
            schedule::schedule_list,
            schedule::schedule_create,
            schedule::schedule_delete,
            schedule::schedule_set_paused,
            health::health,
            health::sdk_version,
            vault::vault_paths,
            tier::tier_status,
            tier::tier_endpoints,
        ])
        .setup(|app| {
            // Resolve the OS-correct app data dir via tauri::Manager::path()
            // and push it into `secrets` so vault + salt land in:
            //   macOS   ~/Library/Application Support/io.userfrm.tddx-store/
            //   Linux   ~/.local/share/io.userfrm.tddx-store/   (XDG)
            //   Windows %LOCALAPPDATA%\io.userfrm.tddx-store\
            //
            // We deliberately use `app_local_data_dir()` (not the
            // roaming `app_data_dir()`) so the Stronghold vault and
            // SQLite queue stay machine-local on Windows — they
            // contain a salt+key derived for THIS machine and a
            // SQLite WAL file that doesn't survive folder roaming.
            let data_dir = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            secrets::set_data_dir(data_dir.clone());

            // Seed Settings defaults from the Tauri-resolved path so the
            // first `settings_get` returns sensible cross-platform paths
            // instead of empty strings.
            let app_state = app.state::<Arc<AppState>>();
            tauri::async_runtime::block_on(async {
                let mut s = app_state.settings.write().await;
                if s.db_path.is_empty() {
                    s.db_path = data_dir.join("queue.db").to_string_lossy().into();
                }
                if s.output_dir.is_empty() {
                    s.output_dir = data_dir.join("data").to_string_lossy().into();
                }
                if s.creds_path.is_empty() {
                    s.creds_path = data_dir.join("creds.txt").to_string_lossy().into();
                }
            });

            // Best-effort tier-table refresh from `docs.thetadata.us`
            // on launch. The build-time table baked from the vendored
            // `spec/openapiv3.yaml` is always the offline fallback —
            // this only adds runtime accuracy when ThetaData publishes
            // a tier change after the binary was built. Failures
            // (offline / DNS down) are logged + ignored.
            tauri::async_runtime::spawn(async {
                match tdds_core::tier::fetch_and_install_remote().await {
                    Ok(n) => tracing::info!(endpoints = n, "tier table refreshed from upstream"),
                    Err(e) => tracing::warn!(error = %e, "tier table refresh failed; using build-time fallback"),
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running TdDx Store");
}

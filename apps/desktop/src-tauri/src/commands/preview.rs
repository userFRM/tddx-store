//! Parquet row-slice preview for the data viewer pane.

use serde::Deserialize;
use tdds_core::preview::{preview as preview_parquet, PreviewResult};

#[derive(Deserialize)]
pub struct PreviewArgs {
    pub path: String,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    100
}

#[tauri::command]
pub async fn parquet_preview(args: PreviewArgs) -> Result<PreviewResult, String> {
    let path = std::path::PathBuf::from(args.path);
    tokio::task::spawn_blocking(move || preview_parquet(&path, args.offset, args.limit))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// A chart's worth of candles from one downloaded file, with the gaps
/// found in it. `step` asks for an interval (`"5m"`, `"1D"`); without
/// one, the finest interval that fits `target` candles is chosen, so
/// the chart fits the width it is drawn at. Runs off the async runtime:
/// a day of trades is millions of rows to read.
#[tauri::command]
pub async fn chart_series(
    path: String,
    step: Option<String>,
    target: Option<usize>,
) -> Result<tdds_core::chart::ChartSeries, String> {
    let path = std::path::PathBuf::from(path);
    let target = target.unwrap_or(tdds_core::chart::DEFAULT_TARGET);
    tokio::task::spawn_blocking(move || tdds_core::chart::series(&path, step.as_deref(), target))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

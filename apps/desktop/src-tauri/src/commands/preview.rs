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

//! Flatfile downloads — the daily bulk snapshots ThetaData distributes
//! as one archive per trading day.
//!
//! The service serves a fixed matrix of five datasets, no more: option
//! `trade_quote` / `open_interest` / `eod` and stock `trade_quote` /
//! `eod`. Per-tick trades, quotes and OHLC bars are market-data
//! endpoints, not flat files, and asking for one returns
//! `INVALID_PARAMS` from the server. `flatfile_datasets` publishes the
//! matrix from the SDK's own `SERVED_DATASETS` so the UI cannot offer a
//! pair that does not exist.
//!
//! Downloading goes through the queue: each served pair is a dataset
//! kind (`tdds_core::spec::flatfile_kinds`), and `DataKind::parse`
//! refuses anything else, so an unserved pair cannot be queued at all.

use serde::Serialize;
use thetadatadx::flatfiles::SERVED_DATASETS;

/// One served flat-file dataset, in the vocabulary the UI renders.
#[derive(Serialize)]
pub struct FlatfileDataset {
    /// `OPTION` | `STOCK`.
    pub sec_type: String,
    /// `trade_quote` | `open_interest` | `eod`.
    pub req_type: String,
    /// The dataset kind that queues it — `flatfile_option_trade_quote`.
    /// Flat files go through the same queue as every other download.
    pub kind: String,
}

/// The flat-file datasets ThetaData actually serves. Derived from the
/// SDK's single source of truth, never from a list kept in the UI.
#[tauri::command]
pub fn flatfile_datasets() -> Vec<FlatfileDataset> {
    SERVED_DATASETS
        .iter()
        .map(|&(sec, req)| FlatfileDataset {
            sec_type: sec.to_string(),
            req_type: req.as_str().to_string(),
            kind: tdds_core::spec::flatfile_kind_name(sec, req),
        })
        .collect()
}

//! Flatfile downloads — the daily bulk snapshots ThetaData distributes
//! as one archive per trading day.
//!
//! The service serves a fixed matrix of five datasets, no more: option
//! `trade_quote` / `open_interest` / `eod` and stock `trade_quote` /
//! `eod`. Per-tick trades, quotes and OHLC bars are market-data
//! endpoints, not flat files, and asking for one returns
//! `INVALID_PARAMS` from the server. `datasets` publishes the matrix
//! from the SDK's own `SERVED_DATASETS` so the UI cannot offer a pair
//! that does not exist, and `flatfile_download` re-checks it so a stale
//! saved search cannot either.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use thetadatadx::flatfiles::{flat_file_serves, FlatFileFormat, ReqType, SecType, SERVED_DATASETS};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct FlatfileArgs {
    pub sec_type: String, // "OPTION" | "STOCK"
    pub req_type: String, // "TRADE_QUOTE" | "OPEN_INTEREST" | "EOD"
    pub date: String,     // YYYYMMDD
    pub output_path: String,
    pub format: String, // "CSV" | "JSONL"
}

/// One served flat-file dataset, in the vocabulary the UI renders.
#[derive(Serialize)]
pub struct FlatfileDataset {
    /// `OPTION` | `STOCK`.
    pub sec_type: String,
    /// `trade_quote` | `open_interest` | `eod`.
    pub req_type: String,
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
        })
        .collect()
}

fn parse_sec(s: &str) -> Result<SecType, String> {
    match s.to_ascii_uppercase().as_str() {
        "OPTION" => Ok(SecType::Option),
        "STOCK" => Ok(SecType::Stock),
        "INDEX" => Ok(SecType::Index),
        other => Err(format!("unknown sec_type {other}")),
    }
}

fn parse_req(s: &str) -> Result<ReqType, String> {
    match s.to_ascii_uppercase().as_str() {
        "TRADE" => Ok(ReqType::Trade),
        "QUOTE" => Ok(ReqType::Quote),
        "TRADE_QUOTE" => Ok(ReqType::TradeQuote),
        "OPEN_INTEREST" => Ok(ReqType::OpenInterest),
        "OHLC" => Ok(ReqType::Ohlc),
        "EOD" => Ok(ReqType::Eod),
        other => Err(format!("unknown req_type {other}")),
    }
}

/// Reject a pair the flat-file service does not serve before spending a
/// round-trip on it, and name the alternative rather than echoing the
/// server's `INVALID_PARAMS`.
fn check_served(sec: SecType, req: ReqType) -> Result<(), String> {
    if flat_file_serves(sec, req) {
        return Ok(());
    }
    let served = SERVED_DATASETS
        .iter()
        .map(|&(s, r)| format!("{s} {}", r.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "{sec} {} is not a flat-file dataset — it is served by the market-data endpoints. \
         Flat files cover: {served}.",
        req.as_str()
    ))
}

fn parse_fmt(s: &str) -> Result<FlatFileFormat, String> {
    match s.to_ascii_uppercase().as_str() {
        "CSV" => Ok(FlatFileFormat::Csv),
        "JSONL" => Ok(FlatFileFormat::Jsonl),
        other => Err(format!("unknown flatfile format {other}")),
    }
}

#[tauri::command]
pub async fn flatfile_download(
    state: State<'_, Arc<AppState>>,
    args: FlatfileArgs,
) -> Result<String, String> {
    let client_guard = state.client.read().await;
    let client = client_guard.as_ref().ok_or("client not connected")?.clone();
    drop(client_guard);
    let sec = parse_sec(&args.sec_type)?;
    let req = parse_req(&args.req_type)?;
    check_served(sec, req)?;
    let fmt = parse_fmt(&args.format)?;
    let path: PathBuf = args.output_path.into();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let result = client
        .raw()
        .flatfile_request(sec, req, &args.date, &path, fmt)
        .await
        .map_err(|e| e.to_string())?;
    Ok(result.to_string_lossy().into_owned())
}

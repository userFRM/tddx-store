//! Flatfile downloads. thetadatadx exposes a separate flatfile API
//! (zip-of-csv per date) for historical bulk pulls; we surface a single
//! convenience command here.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use tauri::State;
use thetadatadx::flatfiles::{FlatFileFormat, ReqType, SecType};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct FlatfileArgs {
    pub sec_type: String, // "OPTION" | "STOCK" | "INDEX"
    pub req_type: String, // "TRADE" | "QUOTE" | "TRADE_QUOTE" | "OPEN_INTEREST" | "OHLC" | "EOD"
    pub date: String,     // YYYYMMDD
    pub output_path: String,
    pub format: String, // "CSV" | "JSONL"
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

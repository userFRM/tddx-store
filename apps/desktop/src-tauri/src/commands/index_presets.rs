//! Index constituent presets via indexkit (S&P 500 / NDX / Sp400 /
//! Sp600 / DJI / RUT). Constituent lists are fetched from sponsor
//! CDNs / SEC EDGAR / GitHub mirrors so the user can bulk-queue
//! downloads across a whole index in one click.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct IndexPresetView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub symbols: Vec<String>,
    pub as_of: Option<String>,
}

#[cfg(feature = "presets")]
async fn fetch_index_constituents(index_id: &str) -> Result<Vec<String>, String> {
    // Free-function API per indexkit docs: one fn per index returning
    // the latest snapshot. Matches "constituents_for(IndexId, ym)" but
    // for the current snapshot we use the per-index helpers.
    let snap = match index_id.to_ascii_lowercase().as_str() {
        "sp500" => indexkit::sp500_latest().await,
        "ndx" => indexkit::ndx_latest().await,
        other => {
            // Fallback: month-keyed helper for indices without a *_latest fn.
            use chrono::Datelike;
            let now = chrono::Utc::now().naive_utc().date();
            let id = match other {
                "sp400" => indexkit::IndexId::Sp400,
                "sp600" => indexkit::IndexId::Sp600,
                "dji" => indexkit::IndexId::Dji,
                "rut" => indexkit::IndexId::Rut,
                _ => return Err(format!("unknown index id {other}")),
            };
            let ym =
                indexkit::YearMonth::new(now.year(), now.month()).map_err(|e| e.to_string())?;
            indexkit::constituents_for(id, ym).await
        }
    }
    .map_err(|e| e.to_string())?;
    let mut tickers: Vec<String> = snap
        .into_iter()
        .filter_map(|c| c.ticker)
        .filter(|t: &String| !t.is_empty())
        .collect();
    tickers.sort();
    tickers.dedup();
    Ok(tickers)
}

#[cfg(not(feature = "presets"))]
async fn fetch_index_constituents(_index_id: &str) -> Result<Vec<String>, String> {
    Err("indexkit feature not built into this binary".into())
}

#[tauri::command]
pub async fn index_constituents(index_id: String) -> Result<Vec<String>, String> {
    fetch_index_constituents(&index_id).await
}

#[tauri::command]
pub async fn index_presets() -> Result<Vec<IndexPresetView>, String> {
    Ok(vec![
        IndexPresetView {
            id: "sp500".into(),
            name: "S&P 500".into(),
            description: "503 large-cap US equities (via IVV)".into(),
            symbols: vec![],
            as_of: None,
        },
        IndexPresetView {
            id: "ndx".into(),
            name: "Nasdaq-100 (NDX)".into(),
            description: "100 largest Nasdaq non-financial issuers (via QQQ)".into(),
            symbols: vec![],
            as_of: None,
        },
        IndexPresetView {
            id: "sp400".into(),
            name: "S&P MidCap 400".into(),
            description: "400 mid-cap US equities (via IJH)".into(),
            symbols: vec![],
            as_of: None,
        },
        IndexPresetView {
            id: "sp600".into(),
            name: "S&P SmallCap 600".into(),
            description: "600 small-cap US equities (via IJR)".into(),
            symbols: vec![],
            as_of: None,
        },
        IndexPresetView {
            id: "dji".into(),
            name: "Dow Jones Industrials".into(),
            description: "30 blue-chip US equities (via DIA)".into(),
            symbols: vec![],
            as_of: None,
        },
        IndexPresetView {
            id: "rut".into(),
            name: "Russell 2000".into(),
            description: "~2000 small-cap US equities (via IWM)".into(),
            symbols: vec![],
            as_of: None,
        },
    ])
}

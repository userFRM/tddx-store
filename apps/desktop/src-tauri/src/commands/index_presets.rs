//! Index constituent presets (S&P 500 / NDX / S&P 400 / S&P 600 / DJI),
//! so the user can bulk-queue downloads across a whole index in one
//! click.
//!
//! Where each list comes from, and why:
//!
//! - S&P 500, Nasdaq-100, Dow: indexkit's `*_latest` snapshots, which
//!   carry tickers.
//! - S&P 400 and S&P 600: indexkit's monthly snapshots for these come
//!   from SEC N-PORT filings, which list CUSIPs and names but no
//!   tickers, and lag the market by a quarter. So these are read from
//!   the tracking ETF's daily holdings file instead (MDY and SPSM on the
//!   SPDR CDN), parsed with indexkit's own SPDR reader.
//!
//! The Russell 2000 is not offered: its only holdings source (iShares
//! IWM) now answers non-browser clients with an HTML page, and indexkit
//! has no snapshot for it either.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct IndexPresetView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub symbols: Vec<String>,
    pub as_of: Option<String>,
}

/// Daily holdings file of the ETF that tracks `index_id`, for the
/// indices whose indexkit snapshot has no tickers.
#[cfg(feature = "presets")]
fn spdr_holdings_url(index_id: &str) -> Option<&'static str> {
    match index_id {
        "sp400" => Some("https://www.ssga.com/us/en/intermediary/library-content/products/fund-data/etfs/us/holdings-daily-us-en-mdy.xlsx"),
        "sp600" => Some("https://www.ssga.com/us/en/intermediary/library-content/products/fund-data/etfs/us/holdings-daily-us-en-spsm.xlsx"),
        _ => None,
    }
}

#[cfg(feature = "presets")]
async fn fetch_spdr(url: &str) -> Result<Vec<indexkit::Constituent>, String> {
    let resp = reqwest::Client::builder()
        .user_agent(indexkit::sponsor::SPONSOR_USER_AGENT)
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?
        .get(url)
        .send()
        .await
        .map_err(|e| format!("holdings file unreachable: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("holdings file returned HTTP {}", resp.status()));
    }
    let body = resp.bytes().await.map_err(|e| e.to_string())?;
    let today = chrono::Utc::now().date_naive();
    indexkit::sponsor::parse_spdr_xlsx(&body, today).map_err(|e| e.to_string())
}

/// Tickers of the newest snapshot in `rows`.
///
/// The `*_latest` helpers return every daily snapshot of the month, so
/// taking all of them would keep names that left the index mid-month
/// (the S&P 500 comes out at ~519 instead of ~503). Cash and other
/// non-equity lines in ETF files are dropped by shape: a listed ticker
/// is letters, digits and at most a class dot (`BRK.B`).
#[cfg(feature = "presets")]
fn newest_tickers(rows: Vec<indexkit::Constituent>) -> Vec<String> {
    let newest = rows.iter().map(|c| c.as_of).max();
    let mut tickers: Vec<String> = rows
        .into_iter()
        .filter(|c| Some(c.as_of) == newest)
        .filter_map(|c| c.ticker)
        .map(|t| t.trim().to_ascii_uppercase())
        .filter(|t| !t.is_empty() && t.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '.'))
        .collect();
    tickers.sort();
    tickers.dedup();
    tickers
}

#[cfg(feature = "presets")]
async fn fetch_index_constituents(index_id: &str) -> Result<Vec<String>, String> {
    let id = index_id.to_ascii_lowercase();
    let rows = if let Some(url) = spdr_holdings_url(&id) {
        fetch_spdr(url).await?
    } else {
        match id.as_str() {
            "sp500" => indexkit::sp500_latest().await,
            "ndx" => indexkit::ndx_latest().await,
            "dji" => indexkit::dji_latest().await,
            other => return Err(format!("unknown index id {other}")),
        }
        .map_err(|e| e.to_string())?
    };
    let tickers = newest_tickers(rows);
    if tickers.is_empty() {
        return Err(format!("the {id} source returned no tickers"));
    }
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
            description: "~503 large-cap US equities".into(),
            symbols: vec![],
            as_of: None,
        },
        IndexPresetView {
            id: "ndx".into(),
            name: "Nasdaq-100 (NDX)".into(),
            description: "100 largest Nasdaq non-financial issuers".into(),
            symbols: vec![],
            as_of: None,
        },
        IndexPresetView {
            id: "sp400".into(),
            name: "S&P MidCap 400".into(),
            description: "400 mid-cap US equities (via MDY holdings)".into(),
            symbols: vec![],
            as_of: None,
        },
        IndexPresetView {
            id: "sp600".into(),
            name: "S&P SmallCap 600".into(),
            description: "600 small-cap US equities (via SPSM holdings)".into(),
            symbols: vec![],
            as_of: None,
        },
        IndexPresetView {
            id: "dji".into(),
            name: "Dow Jones Industrials".into(),
            description: "30 blue-chip US equities".into(),
            symbols: vec![],
            as_of: None,
        },
    ])
}

#[cfg(all(test, feature = "presets"))]
mod tests {
    use super::*;

    /// Every offered preset resolves to a plausible ticker count.
    /// Network-bound, so opt-in: `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn every_preset_loads() {
        for p in index_presets().await.unwrap() {
            let t = fetch_index_constituents(&p.id).await.unwrap();
            let (lo, hi) = match p.id.as_str() {
                "sp500" => (495, 510),
                "ndx" => (98, 104),
                "sp400" => (395, 405),
                "sp600" => (595, 610),
                "dji" => (30, 30),
                _ => unreachable!(),
            };
            assert!(
                (lo..=hi).contains(&t.len()),
                "{}: {} tickers",
                p.id,
                t.len()
            );
            println!("{}: {} tickers", p.id, t.len());
        }
    }
}

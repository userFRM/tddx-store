//! Index constituent presets via indexkit, so the user can bulk-queue
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
    let id: indexkit::IndexId = index_id.parse()?;
    let snap = indexkit::latest(id).await.map_err(|e| e.to_string())?;
    let mut tickers: Vec<String> = snap
        .constituents
        .into_iter()
        .filter_map(|c| c.ticker)
        .filter(|t| !t.is_empty())
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
    let preset = |id: &str, name: &str, description: &str| IndexPresetView {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        symbols: vec![],
        as_of: None,
    };
    Ok(vec![
        preset("sp500", "S&P 500", "~503 large-cap US equities"),
        preset(
            "ndx",
            "Nasdaq-100 (NDX)",
            "100 largest Nasdaq non-financial issuers",
        ),
        preset("sp400", "S&P MidCap 400", "400 mid-cap US equities"),
        preset("sp600", "S&P SmallCap 600", "600 small-cap US equities"),
        preset("dji", "Dow Jones Industrials", "30 blue-chip US equities"),
        preset("rut", "Russell 2000", "~2000 small-cap US equities"),
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
            let n = fetch_index_constituents(&p.id).await.unwrap().len();
            let (lo, hi) = match p.id.as_str() {
                "sp500" => (495, 510),
                "ndx" => (98, 104),
                "sp400" => (395, 405),
                "sp600" => (595, 610),
                "dji" => (30, 30),
                "rut" => (1900, 2050),
                _ => unreachable!(),
            };
            println!("{}: {n} tickers", p.id);
            assert!((lo..=hi).contains(&n), "{}: {n} tickers", p.id);
        }
    }
}

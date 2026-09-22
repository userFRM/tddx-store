//! On-disk coverage report + DuckDB hint.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;
use tdds_core::{coverage, format::OutputFormat, DataKind, DataSpec};

use crate::state::AppState;

#[tauri::command]
pub async fn coverage_report(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let cfg = state.settings.read().await.clone();
    let cov = coverage::scan(&PathBuf::from(&cfg.output_dir)).map_err(|e| e.to_string())?;
    Ok(cov
        .into_iter()
        .map(|c| {
            serde_json::json!({
                "kind": c.kind.as_str(),
                "symbol": c.symbol,
                "files": c.dates.len(),
                "bytes": c.bytes,
                "first": c.dates.first().map(|d| d.format("%Y-%m-%d").to_string()),
                "last": c.dates.last().map(|d| d.format("%Y-%m-%d").to_string()),
                // Every date actually present, not just the span. The
                // heatmap needs these: inferring "every weekday between
                // first and last" paints gaps as though they were files.
                "dates": c.dates.iter().map(|d| d.format("%Y-%m-%d").to_string()).collect::<Vec<_>>(),
                "format": c.format.extension(),
            })
        })
        .collect())
}

/// The set on disk for one (dataset, symbol), and the trading days
/// inside its span that are not downloaded.
struct Gaps {
    have: coverage::Coverage,
    dates: Vec<chrono::NaiveDate>,
}

/// Which trading days are missing from a set's own span.
///
/// The span is what the library already holds: first file to last file.
/// Which days exist inside it comes from the vendor's own calendar, so a
/// market holiday is never counted as a gap — the reason this asks the
/// server rather than assuming every weekday is a trading day.
async fn find_gaps(state: &AppState, kind: &str, symbol: &str) -> Result<Gaps, String> {
    let cfg = state.settings.read().await.clone();
    let client = {
        let g = state.client.read().await;
        g.as_ref().ok_or("client not connected")?.clone()
    };
    let data_kind = DataKind::parse(kind).ok_or_else(|| format!("unknown dataset {kind}"))?;

    let have = coverage::scan(&PathBuf::from(&cfg.output_dir))
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|c| c.kind == data_kind && c.symbol.eq_ignore_ascii_case(symbol))
        .ok_or_else(|| format!("nothing on disk for {symbol} {kind}"))?;

    let (Some(start), Some(end)) = (have.dates.first().copied(), have.dates.last().copied()) else {
        return Ok(Gaps {
            have,
            dates: Vec::new(),
        });
    };
    let server_days = client
        .trading_days(symbol, start, end)
        .await
        .map_err(|e| e.to_string())?;
    let dates = coverage::missing(&server_days, &have.dates, start, end);
    Ok(Gaps { have, dates })
}

/// The exact trading days missing from a set's span, as `YYYY-MM-DD`.
///
/// Read-only, so the UI can show which days are absent before deciding
/// to fetch them. The count that comes back is the same one
/// [`requeue_missing_dates`] would act on — one number, from one source,
/// rather than a client-side guess that disagrees with the server.
#[tauri::command]
pub async fn missing_dates(
    state: State<'_, Arc<AppState>>,
    kind: String,
    symbol: String,
) -> Result<Vec<String>, String> {
    Ok(find_gaps(&state, &kind, &symbol)
        .await?
        .dates
        .iter()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .collect())
}

/// Queue every trading day missing from a set's span. Returns how many
/// tasks were added, in the format the set already uses.
#[tauri::command]
pub async fn requeue_missing_dates(
    state: State<'_, Arc<AppState>>,
    kind: String,
    symbol: String,
) -> Result<usize, String> {
    let cfg = state.settings.read().await.clone();
    let queue = {
        let g = state.queue.read().await;
        g.as_ref().ok_or("queue not opened")?.clone()
    };
    let data_kind = DataKind::parse(&kind).ok_or_else(|| format!("unknown dataset {kind}"))?;
    let gaps = find_gaps(&state, &kind, &symbol).await?;
    let format: OutputFormat = gaps.have.format;

    for date in &gaps.dates {
        let spec = DataSpec {
            kind: data_kind.clone(),
            symbol: symbol.clone(),
            date: *date,
            interval: None,
            expiration: "*".into(),
            strike: "*".into(),
            right: "both".into(),
            end_date: None,
            transforms: Default::default(),
            extra: Default::default(),
        };
        queue
            .enqueue(spec, format, &cfg.output_dir, 0)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(gaps.dates.len())
}

/// Generates a DuckDB SQL bootstrap that scans the user's parquet
/// output directory and registers each `(symbol, kind)` partition as a
/// view. Returns structured fields so the frontend can present them
/// safely (escaped for DuckDB string literals at the boundary, never
/// pre-built into a "ready-to-paste shell command" with raw user paths).
///
/// SQL string literal escaping: SQLite/DuckDB take single-quoted string
/// literals where `''` is the escape for a literal `'`. We escape only
/// that — not shell metachars — because the SQL is run inside DuckDB,
/// not through a shell.
#[tauri::command]
pub async fn duckdb_command(output_dir: String) -> Result<serde_json::Value, String> {
    let dir = PathBuf::from(&output_dir);
    if !dir.exists() {
        return Err(format!("output dir doesn't exist: {output_dir}"));
    }
    let cov = coverage::scan(&dir).map_err(|e| e.to_string())?;
    fn sql_escape(s: &str) -> String {
        s.replace('\'', "''")
    }
    let dir_sql = sql_escape(&dir.to_string_lossy());
    let mut sql = String::from("INSTALL parquet; LOAD parquet;\n");
    let mut seen_kinds = std::collections::BTreeSet::new();
    for c in &cov {
        if seen_kinds.insert(c.kind.as_str()) {
            sql.push_str(&format!(
                "CREATE OR REPLACE VIEW {kind} AS \
                 SELECT * FROM read_parquet('{dir}/{kind}/*.parquet');\n",
                kind = c.kind.as_str(),
                dir = dir_sql,
            ));
        }
    }
    Ok(serde_json::json!({
        "sql": sql,
        "path": dir.to_string_lossy(),
        "hint": "duckdb < /tmp/tdds_duckdb_init.sql  (or paste sql interactively)",
    }))
}

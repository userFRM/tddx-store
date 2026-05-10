//! On-disk coverage report + DuckDB hint.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;
use tdds_core::coverage;

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
            })
        })
        .collect())
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

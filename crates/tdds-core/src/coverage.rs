//! Walk an output directory and report which (kind, symbol, date) files exist.

use chrono::NaiveDate;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::spec::DataKind;

#[derive(Debug, Serialize, Clone)]
pub struct Coverage {
    pub kind: DataKind,
    pub symbol: String,
    pub dates: Vec<NaiveDate>,
    pub bytes: u64,
}

/// Path layout: `<root>/<kind>/<symbol>_<kind>_<YYYYMMDD>.<ext>`.
pub fn dataset_path(root: &Path, kind: DataKind, symbol: &str, ymd: &str, ext: &str) -> PathBuf {
    root.join(kind.as_str()).join(format!(
        "{}_{}_{}.{}",
        symbol.to_lowercase(),
        kind.as_str(),
        ymd,
        ext
    ))
}

/// Returns one `Coverage` per (kind, symbol) seen under `root`.
pub fn scan(root: &Path) -> crate::Result<Vec<Coverage>> {
    let mut out: std::collections::BTreeMap<(DataKind, String), (BTreeSet<NaiveDate>, u64)> =
        std::collections::BTreeMap::new();
    if !root.exists() {
        return Ok(vec![]);
    }
    for kind_entry in std::fs::read_dir(root)? {
        let kind_entry = kind_entry?;
        if !kind_entry.file_type()?.is_dir() {
            continue;
        }
        let kind_name = kind_entry.file_name().to_string_lossy().into_owned();
        let Some(kind) = DataKind::parse(&kind_name) else {
            continue;
        };
        for f in std::fs::read_dir(kind_entry.path())? {
            let f = f?;
            let name = f.file_name().to_string_lossy().into_owned();
            // expected: <symbol>_<kind>_<YYYYMMDD>.<ext>
            let stem = match name.rsplit_once('.') {
                Some((s, _)) => s,
                None => continue,
            };
            let parts: Vec<&str> = stem.split('_').collect();
            if parts.len() < 3 {
                continue;
            }
            let ymd_str = parts[parts.len() - 1];
            let Ok(d) = NaiveDate::parse_from_str(ymd_str, "%Y%m%d") else {
                continue;
            };
            // <symbol>_<kind...>_<YYYYMMDD>. Symbol = first underscore-segment.
            let symbol = parts[0].to_uppercase();
            let entry = out
                .entry((kind, symbol))
                .or_insert_with(|| (BTreeSet::new(), 0u64));
            entry.0.insert(d);
            entry.1 += f.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    Ok(out
        .into_iter()
        .map(|((kind, symbol), (dates, bytes))| Coverage {
            kind,
            symbol,
            dates: dates.into_iter().collect(),
            bytes,
        })
        .collect())
}

/// Subset of `[start, end]` trading days (server-truth) NOT yet on disk.
pub fn missing(
    server_days: &[NaiveDate],
    have: &[NaiveDate],
    start: NaiveDate,
    end: NaiveDate,
) -> Vec<NaiveDate> {
    let have: BTreeSet<NaiveDate> = have.iter().copied().collect();
    server_days
        .iter()
        .filter(|d| **d >= start && **d <= end && !have.contains(d))
        .copied()
        .collect()
}

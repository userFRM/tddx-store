//! Walk an output directory and report which (kind, symbol, date) files exist.

use chrono::NaiveDate;
use serde::Serialize;
use std::collections::BTreeSet;

use chrono::Datelike;
use std::path::{Path, PathBuf};

use crate::format::OutputFormat;
use crate::spec::{DataKind, DataSpec};

#[derive(Debug, Serialize, Clone)]
pub struct Coverage {
    pub kind: DataKind,
    pub symbol: String,
    pub dates: Vec<NaiveDate>,
    /// Files on disk. Not `dates.len()`: one range file covers a year.
    pub files: usize,
    pub bytes: u64,
    /// File extension the existing files use, so refilling a gap writes
    /// the same format as the rest of the set rather than the default.
    pub format: OutputFormat,
    /// The file holding the latest date, so the Library can open
    /// something without reconstructing a filename it cannot know —
    /// qualifiers and windows are part of the name.
    pub latest_path: Option<String>,
}

/// Where one work unit's file lives:
/// `<root>/<dataset>/<symbol>_<dataset>[_<qualifier>]_<YYYYMMDD>.<ext>`.
///
/// The date stays last so [`scan`] can read it off the end whatever the
/// qualifier contains, and the qualifier is empty for a whole-chain
/// pull so existing libraries keep their filenames.
pub fn dataset_path(root: &Path, spec: &DataSpec, ext: &str) -> PathBuf {
    root.join(spec.kind.as_str())
        .join(format!("{}.{}", spec.file_stem(), ext))
}

/// What the scan accumulates per (dataset, symbol) before it becomes a
/// [`Coverage`]: the dates seen, their total size, and the format of the
/// first file encountered.
#[derive(Default)]
struct Tally {
    dates: BTreeSet<NaiveDate>,
    files: usize,
    bytes: u64,
    format: Option<OutputFormat>,
    latest: Option<(NaiveDate, PathBuf)>,
}

/// Returns one `Coverage` per (kind, symbol) seen under `root`.
pub fn scan(root: &Path) -> crate::Result<Vec<Coverage>> {
    let mut out: std::collections::BTreeMap<(DataKind, String), Tally> =
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
            let (stem, ext) = match name.rsplit_once('.') {
                Some(parts) => parts,
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
            let entry = out.entry((kind.clone(), symbol)).or_default();
            // A file from a range endpoint covers a whole window, named
            // by its start with the end as a `to<YYYYMMDD>` qualifier.
            // Reading only the start made a year of data count as one
            // day, so the Library understated every span and "check
            // gaps" reported the rest of the year as missing.
            match range_end(&parts) {
                Some(end) if end > d => {
                    let mut day = d;
                    while day <= end {
                        if !matches!(day.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun) {
                            entry.dates.insert(day);
                        }
                        match day.succ_opt() {
                            Some(next) => day = next,
                            None => break,
                        }
                    }
                }
                _ => {
                    entry.dates.insert(d);
                }
            }
            entry.files += 1;
            entry.bytes += f.metadata().map(|m| m.len()).unwrap_or(0);
            if entry.format.is_none() {
                entry.format = OutputFormat::parse(ext);
            }
            if entry.latest.as_ref().is_none_or(|(latest, _)| d >= *latest) {
                entry.latest = Some((d, f.path()));
            }
        }
    }
    Ok(out
        .into_iter()
        .map(|((kind, symbol), tally)| Coverage {
            kind,
            symbol,
            dates: tally.dates.into_iter().collect(),
            files: tally.files,
            bytes: tally.bytes,
            format: tally.format.unwrap_or_default(),
            latest_path: tally.latest.map(|(_, p)| p.to_string_lossy().into_owned()),
        })
        .collect())
}

/// The `to<YYYYMMDD>` qualifier a range file carries, if any.
fn range_end(parts: &[&str]) -> Option<NaiveDate> {
    parts
        .iter()
        .rev()
        .skip(1)
        .find_map(|p| p.strip_prefix("to"))
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y%m%d").ok())
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

#[cfg(test)]
mod range_tests {
    use super::*;

    #[test]
    fn a_range_file_covers_its_whole_window() {
        let dir = std::env::temp_dir().join(format!("tdds-cov-{}", std::process::id()));
        let kind_dir = dir.join("stock_history_eod");
        std::fs::create_dir_all(&kind_dir).unwrap();
        // Mon 2024-01-01 .. Fri 2024-01-12: ten weekdays in one file.
        std::fs::write(
            kind_dir.join("spy_stock_history_eod_to20240112_20240101.parquet"),
            b"x",
        )
        .unwrap();

        let cov = scan(&dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(cov.len(), 1);
        assert_eq!(cov[0].dates.len(), 10, "every weekday in the window");
        assert_eq!(cov[0].files, 1, "but it is still one file");
        assert_eq!(
            cov[0].dates.last(),
            NaiveDate::from_ymd_opt(2024, 1, 12).as_ref()
        );
    }
}

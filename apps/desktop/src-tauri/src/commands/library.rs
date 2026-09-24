//! Finder-style operations on the files in the output directory: list
//! a set's files, total up a selection, and move it to the Trash.
//!
//! Every path the frontend names is checked against the output
//! directory before anything is touched. The selection arrives as
//! whole sets (dataset, symbol) and single files; the backend resolves
//! both to files itself, so the count the confirmation shows and the
//! files that are removed come from the same list.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use tdds_core::{coverage, DataKind, LibraryFile};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct SetRef {
    pub kind: String,
    pub symbol: String,
}

/// What the user selected: whole sets, and files picked individually.
/// A file inside a selected set is counted once.
#[derive(Deserialize, Default)]
pub struct Targets {
    #[serde(default)]
    pub sets: Vec<SetRef>,
    #[serde(default)]
    pub files: Vec<String>,
}

#[derive(Serialize)]
pub struct Resolved {
    pub files: usize,
    pub bytes: u64,
}

#[derive(Serialize)]
pub struct TrashResult {
    pub files: usize,
    pub bytes: u64,
}

async fn output_root(state: &AppState) -> Result<PathBuf, String> {
    let dir = state.settings.read().await.output_dir.clone();
    if dir.is_empty() {
        return Err("no output directory set".into());
    }
    std::fs::canonicalize(&dir).map_err(|e| format!("output directory {dir}: {e}"))
}

/// `path`, canonical, if it is a regular file somewhere under `root`.
/// Symlinks are resolved first, so a link pointing out of the library
/// is refused rather than followed.
fn inside(root: &Path, path: &str) -> Result<PathBuf, String> {
    let p = std::fs::canonicalize(path).map_err(|e| format!("{path}: {e}"))?;
    if !p.starts_with(root) || p == root {
        return Err(format!("{path} is outside the output directory"));
    }
    if !p.is_file() {
        return Err(format!("{path} is not a file"));
    }
    Ok(p)
}

fn resolve(root: &Path, targets: &Targets) -> Result<Vec<(PathBuf, u64)>, String> {
    let mut paths: BTreeSet<PathBuf> = BTreeSet::new();
    for set in &targets.sets {
        let kind =
            DataKind::parse(&set.kind).ok_or_else(|| format!("unknown dataset {}", set.kind))?;
        for f in coverage::files(root, &kind, &set.symbol).map_err(|e| e.to_string())? {
            paths.insert(inside(root, &f.path)?);
        }
    }
    for f in &targets.files {
        paths.insert(inside(root, f)?);
    }
    Ok(paths
        .into_iter()
        .map(|p| {
            let bytes = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            (p, bytes)
        })
        .collect())
}

/// The files of one (dataset, symbol), newest first.
#[tauri::command]
pub async fn library_files(
    state: State<'_, Arc<AppState>>,
    kind: String,
    symbol: String,
) -> Result<Vec<LibraryFile>, String> {
    let root = output_root(&state).await?;
    let kind = DataKind::parse(&kind).ok_or_else(|| format!("unknown dataset {kind}"))?;
    coverage::files(&root, &kind, &symbol).map_err(|e| e.to_string())
}

/// How many files, and how many bytes, a selection comes to. The
/// confirmation shows these before anything is removed.
#[tauri::command]
pub async fn library_resolve(
    state: State<'_, Arc<AppState>>,
    targets: Targets,
) -> Result<Resolved, String> {
    let root = output_root(&state).await?;
    let files = resolve(&root, &targets)?;
    Ok(Resolved {
        files: files.len(),
        bytes: files.iter().map(|(_, b)| b).sum(),
    })
}

/// Move a selection to the Trash. Recoverable from the Trash in Finder
/// like anything else deleted there; nothing is unlinked outright.
#[tauri::command]
pub async fn library_trash(
    state: State<'_, Arc<AppState>>,
    targets: Targets,
) -> Result<TrashResult, String> {
    let root = output_root(&state).await?;
    let files = resolve(&root, &targets)?;
    if files.is_empty() {
        return Ok(TrashResult { files: 0, bytes: 0 });
    }
    let bytes = files.iter().map(|(_, b)| b).sum();
    let paths: Vec<PathBuf> = files.into_iter().map(|(p, _)| p).collect();
    let n = paths.len();
    tokio::task::spawn_blocking(move || trash_all(&paths))
        .await
        .map_err(|e| e.to_string())??;
    Ok(TrashResult { files: n, bytes })
}

fn trash_all(paths: &[PathBuf]) -> Result<(), String> {
    let ctx = {
        #[allow(unused_mut)]
        let mut ctx = trash::TrashContext::default();
        // The default on macOS asks Finder over AppleScript, which pops
        // an automation-permission prompt the first time. The file
        // manager API needs no permission.
        #[cfg(target_os = "macos")]
        {
            use trash::macos::{DeleteMethod, TrashContextExtMacos};
            ctx.set_delete_method(DeleteMethod::NsFileManager);
        }
        ctx
    };
    ctx.delete_all(paths).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_paths_outside_the_library() {
        let base = std::env::temp_dir().join(format!("tdds-lib-{}", std::process::id()));
        let root = base.join("out");
        std::fs::create_dir_all(root.join("stock_history_eod")).unwrap();
        let inside_file = root.join("stock_history_eod/spy_stock_history_eod_20240102.parquet");
        let outside_file = base.join("secret.txt");
        std::fs::write(&inside_file, b"x").unwrap();
        std::fs::write(&outside_file, b"x").unwrap();
        let root = std::fs::canonicalize(&root).unwrap();

        assert!(inside(&root, inside_file.to_str().unwrap()).is_ok());
        assert!(inside(&root, outside_file.to_str().unwrap()).is_err());
        let sneaky = root.join("stock_history_eod/../../secret.txt");
        assert!(inside(&root, sneaky.to_str().unwrap()).is_err());
        assert!(
            inside(&root, root.to_str().unwrap()).is_err(),
            "never the root itself"
        );

        let t = Targets {
            sets: vec![SetRef {
                kind: "stock_history_eod".into(),
                symbol: "spy".into(),
            }],
            files: vec![inside_file.to_string_lossy().into_owned()],
        };
        let got = resolve(&root, &t).unwrap();
        std::fs::remove_dir_all(&base).ok();
        assert_eq!(got.len(), 1, "a file in a selected set counts once");
    }
}

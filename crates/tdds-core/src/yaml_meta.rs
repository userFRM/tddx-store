//! Per-endpoint catalogue metadata sourced from
//! `https://docs.thetadata.us/openapiv3.yaml` — single source of truth
//! for the dataset store. Build-time bake from the vendored copy
//! lives in `tier::ENDPOINT_META_TABLE`; runtime override fetched on
//! launch lives in this module's `RUNTIME_META`.
//!
//! Use `catalogue()` wherever the UI needs a human-readable summary,
//! description or tag — never hand-code copy in the frontend that is
//! already in the yaml.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use serde::Serialize;

use crate::tier::{Tier, ENDPOINT_META_TABLE};

#[derive(Debug, Clone, Serialize)]
pub struct EndpointMeta {
    pub operation_id: String,
    pub summary: String,
    pub description: String,
    pub tag: String,
    pub min_tier: Option<Tier>,
}

static RUNTIME_META: OnceLock<RwLock<Option<HashMap<String, EndpointMeta>>>> = OnceLock::new();

/// Whole catalogue. Runtime override merges with the build-time table —
/// runtime entries win, build-time entries fill gaps for ops the
/// upstream yaml dropped (defensive against accidental spec regressions).
pub fn catalogue() -> Vec<EndpointMeta> {
    let mut by_op: HashMap<String, EndpointMeta> = HashMap::new();
    for &(op, summary, description, tag, min_tier) in ENDPOINT_META_TABLE {
        by_op.insert(
            op.to_string(),
            EndpointMeta {
                operation_id: op.to_string(),
                summary: summary.to_string(),
                description: description.to_string(),
                tag: tag.to_string(),
                min_tier,
            },
        );
    }
    if let Some(cell) = RUNTIME_META.get() {
        if let Ok(guard) = cell.read() {
            if let Some(map) = guard.as_ref() {
                for (op, meta) in map {
                    by_op.insert(op.clone(), meta.clone());
                }
            }
        }
    }
    let mut out: Vec<EndpointMeta> = by_op.into_values().collect();
    out.sort_by(|a, b| a.operation_id.cmp(&b.operation_id));
    out
}

/// Replace the runtime metadata table with a freshly-parsed map from
/// the upstream yaml. Called by `tier::fetch_and_install_remote` when
/// it pulls fresh spec.
pub fn install_runtime_meta(map: HashMap<String, EndpointMeta>) {
    let cell = RUNTIME_META.get_or_init(|| RwLock::new(None));
    if let Ok(mut guard) = cell.write() {
        *guard = Some(map);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The desktop Browse catalogue inner-joins the SDK endpoint
    /// registry against this table on `operation_id`, and silently
    /// drops anything that misses. A registry endpoint with no entry
    /// here would therefore vanish from the UI with no error — the
    /// user simply could not download it. Assert the join is total.
    #[test]
    fn every_registry_endpoint_has_catalogue_metadata() {
        let have: HashMap<&str, ()> = ENDPOINT_META_TABLE
            .iter()
            .map(|&(op, ..)| (op, ()))
            .collect();
        let missing: Vec<&str> = thetadatadx::ENDPOINTS
            .iter()
            .map(|m| m.name)
            .filter(|n| !have.contains_key(n))
            .collect();
        assert!(
            missing.is_empty(),
            "{} registry endpoint(s) would disappear from Browse: {missing:?}",
            missing.len()
        );
    }
}

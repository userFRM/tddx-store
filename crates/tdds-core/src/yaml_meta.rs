//! Per-endpoint catalogue metadata sourced from
//! `https://docs.thetadata.us/openapiv3.yaml` — single source of truth
//! for the dataset store. Build-time bake from the vendored copy
//! lives in `tier::ENDPOINT_META_TABLE`; runtime override fetched on
//! launch lives in this module's `RUNTIME_META`.
//!
//! Use `endpoint_meta(op)` everywhere that needs a human-readable
//! description, summary, or UI tag — never hand-code copy in the FE
//! that's already in the yaml.

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

/// Lookup metadata for one endpoint by operationId. Runtime override
/// (from the freshly-fetched yaml) takes priority over the build-time
/// table; falls through to `None` for unknown ops.
pub fn endpoint_meta(operation_id: &str) -> Option<EndpointMeta> {
    if let Some(cell) = RUNTIME_META.get() {
        if let Ok(guard) = cell.read() {
            if let Some(map) = guard.as_ref() {
                if let Some(m) = map.get(operation_id) {
                    return Some(m.clone());
                }
            }
        }
    }
    for &(op, summary, description, tag, min_tier) in ENDPOINT_META_TABLE {
        if op == operation_id {
            return Some(EndpointMeta {
                operation_id: op.to_string(),
                summary: summary.to_string(),
                description: description.to_string(),
                tag: tag.to_string(),
                min_tier,
            });
        }
    }
    None
}

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

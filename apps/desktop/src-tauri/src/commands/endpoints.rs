//! Generic endpoint dispatch surface — covers all 61 thetadatadx
//! endpoints via the registry. List endpoints get a JSON-friendly
//! `Vec<String>` return; everything else writes a tick batch to disk.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use tdds_core::{
    all_endpoints, dispatch_to_file, endpoint_catalogue, endpoint_meta, find_endpoint,
    format::OutputFormat, EndpointInfo, EndpointMeta, EndpointSpec,
};

use crate::state::AppState;

#[tauri::command]
pub async fn endpoints_list() -> Result<Vec<EndpointInfo>, String> {
    Ok(all_endpoints())
}

#[tauri::command]
pub async fn endpoints_get(name: String) -> Result<Option<EndpointInfo>, String> {
    Ok(find_endpoint(&name))
}

#[derive(Deserialize)]
pub struct InvokeArgs {
    pub endpoint: String,
    pub args: std::collections::BTreeMap<String, String>,
    pub format: String,
    pub output_path: String,
    #[serde(default)]
    pub timeout_ms: u64,
}

#[derive(Deserialize)]
pub struct ListQueryArgs {
    pub endpoint: String,
    pub args: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub timeout_ms: u64,
}

/// Run a `*_list_*` endpoint (stock_list_symbols, option_list_expirations,
/// option_list_strikes, …) and return the resulting `Vec<String>`
/// directly to the frontend so dropdown / multi-select widgets can
/// populate dynamically. Errors when the endpoint is not a list endpoint.
#[tauri::command]
pub async fn list_query(
    state: State<'_, Arc<AppState>>,
    args: ListQueryArgs,
) -> Result<Vec<String>, String> {
    let client_guard = state.client.read().await;
    let client = client_guard.as_ref().ok_or("client not connected")?.clone();
    drop(client_guard);
    let mut spec = EndpointSpec::new(args.endpoint);
    for (k, v) in args.args {
        spec.args.insert(k, v);
    }
    spec.timeout_ms = args.timeout_ms;
    let raw = tdds_core::dispatch_raw(&client, &spec)
        .await
        .map_err(|e| e.to_string())?;
    match raw {
        thetadatadx::EndpointOutput::StringList(v) => Ok(v),
        other => Err(format!(
            "endpoint {} returned non-list output {:?}",
            spec.endpoint, other
        )),
    }
}

/// One-shot dispatch of any registered endpoint. Writes result to
/// `output_path` in the chosen format. Returns row count.
#[tauri::command]
pub async fn endpoint_invoke(
    state: State<'_, Arc<AppState>>,
    args: InvokeArgs,
) -> Result<usize, String> {
    let client_guard = state.client.read().await;
    let client = client_guard.as_ref().ok_or("client not connected")?.clone();
    drop(client_guard);
    let format = OutputFormat::parse(&args.format)
        .ok_or_else(|| format!("unknown format {}", args.format))?;
    let mut spec = EndpointSpec::new(args.endpoint);
    for (k, v) in args.args {
        spec.args.insert(k, v);
    }
    spec.timeout_ms = args.timeout_ms;
    dispatch_to_file(
        &client,
        &spec,
        std::path::Path::new(&args.output_path),
        format,
    )
    .await
    .map_err(|e| e.to_string())
}

/// Merged per-endpoint catalogue: thetadatadx registry (params, return
/// type, REST path) + yaml metadata (summary, description, tag,
/// min_tier). FE renders the dataset store from this — no
/// hand-coded `DATASETS` array. Drops endpoints not present in the
/// yaml (snapshot-style + unrecognized ops).
#[derive(Serialize)]
pub struct CatalogueEntry {
    pub name: String,
    pub category: String,
    pub subcategory: String,
    pub rest_path: String,
    pub returns: String,
    pub params: Vec<tdds_core::ParamInfo>,
    pub summary: String,
    pub description: String,
    pub tag: String,
    pub min_tier: Option<tdds_core::Tier>,
}

#[tauri::command]
pub async fn dataset_catalogue() -> Result<Vec<CatalogueEntry>, String> {
    let yaml: std::collections::HashMap<String, EndpointMeta> = endpoint_catalogue()
        .into_iter()
        .map(|m| (m.operation_id.clone(), m))
        .collect();
    let mut out: Vec<CatalogueEntry> = Vec::new();
    for info in all_endpoints() {
        let Some(meta) = yaml.get(&info.name) else {
            continue;
        };
        out.push(CatalogueEntry {
            name: info.name.clone(),
            category: info.category.clone(),
            subcategory: info.subcategory.clone(),
            rest_path: info.rest_path.clone(),
            returns: info.returns.clone(),
            params: info.params.clone(),
            summary: meta.summary.clone(),
            description: meta.description.clone(),
            tag: meta.tag.clone(),
            min_tier: meta.min_tier,
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn dataset_metadata(operation_id: String) -> Result<Option<EndpointMeta>, String> {
    Ok(endpoint_meta(&operation_id))
}

//! Generic endpoint dispatch + registry exposure.
//!
//! Wraps `thetadatadx::invoke_endpoint` and `EndpointArgs` so the rest of
//! tdds-core can drive every historical / snapshot /
//! list / at_time / greeks endpoints from a single `EndpointSpec` value.
//!
//! `dispatch_to_arrow` returns the Arrow `RecordBatch` (or empty) so the
//! caller can pipe it into any `format::write_batch` target.

use std::path::Path;

use arrow_array::RecordBatch;
use thetadatadx::{
    by_category, endpoint::invoke_endpoint, find, EndpointArgs, EndpointMeta, EndpointOutput,
    ENDPOINTS,
};

use crate::client::Client;
use crate::format::{write_batch, OutputFormat};
use crate::spec::EndpointSpec;

/// Static metadata for one endpoint param, JSON-friendly for the UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ParamInfo {
    pub name: String,
    pub description: String,
    pub param_type: String,
    pub required: bool,
}

/// JSON-friendly endpoint descriptor. Frontend uses this to build the
/// "Add download" form dynamically — one form per endpoint.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EndpointInfo {
    pub name: String,
    pub description: String,
    pub category: String,
    pub subcategory: String,
    pub rest_path: String,
    pub returns: String,
    pub params: Vec<ParamInfo>,
}

impl From<&EndpointMeta> for EndpointInfo {
    fn from(m: &EndpointMeta) -> Self {
        Self {
            name: m.name.to_string(),
            description: m.description.to_string(),
            category: m.category.to_string(),
            subcategory: m.subcategory.to_string(),
            rest_path: m.rest_path.to_string(),
            returns: format!("{:?}", m.returns),
            params: m
                .params
                .iter()
                .map(|p| ParamInfo {
                    name: p.name.to_string(),
                    description: p.description.to_string(),
                    param_type: format!("{:?}", p.param_type),
                    required: p.required,
                })
                .collect(),
        }
    }
}

/// Every registered endpoint, sorted by category > subcategory > name.
pub fn all_endpoints() -> Vec<EndpointInfo> {
    let mut v: Vec<EndpointInfo> = ENDPOINTS.iter().map(Into::into).collect();
    v.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then(a.subcategory.cmp(&b.subcategory))
            .then(a.name.cmp(&b.name))
    });
    v
}

pub fn endpoints_by_category(cat: &str) -> Vec<EndpointInfo> {
    by_category(cat).iter().map(|m| (*m).into()).collect()
}

pub fn find_endpoint(name: &str) -> Option<EndpointInfo> {
    find(name).map(Into::into)
}

/// Run `spec` against the live MDDS gRPC and write the result as `fmt` to
/// `path`. Returns rows written. Empty results return `Ok(0)` and leave
/// `path` untouched.
pub async fn dispatch_to_file(
    client: &Client,
    spec: &EndpointSpec,
    path: &Path,
    fmt: OutputFormat,
) -> crate::Result<usize> {
    let Some(batch_opt) = dispatch_to_arrow(client, spec).await? else {
        return Ok(0);
    };
    let rows = batch_opt.num_rows();
    if rows == 0 {
        return Ok(0);
    }
    write_batch(&batch_opt, path, fmt)?;
    Ok(rows)
}

/// Run `spec` and convert the `EndpointOutput` to a `RecordBatch`.
/// `Ok(None)` means the endpoint returned a non-tick output (e.g.
/// `StringList`) that has no Arrow representation — caller can still
/// inspect it via `dispatch_raw` if needed.
pub async fn dispatch_to_arrow(
    client: &Client,
    spec: &EndpointSpec,
) -> crate::Result<Option<RecordBatch>> {
    let output = dispatch_raw(client, spec).await?;
    Ok(arrow_from_output(&output))
}

/// Low-level: invoke the endpoint and return the typed `EndpointOutput`.
pub async fn dispatch_raw(client: &Client, spec: &EndpointSpec) -> crate::Result<EndpointOutput> {
    let meta = find(&spec.endpoint)
        .ok_or_else(|| crate::Error::Other(format!("unknown endpoint '{}'", spec.endpoint)))?;
    let mut args = EndpointArgs::new();
    if spec.timeout_ms > 0 {
        args = args.with_timeout_ms(spec.timeout_ms);
    }
    for p in meta.params {
        if let Some(raw) = spec.args.get(p.name) {
            // `interval` is the one argument whose accepted spelling
            // changed with the v3 API (`0` → `tick`, `60s` → `1m`, …).
            // Normalizing here covers every caller — the worker, the
            // endpoint runner, list queries and schedules — so no
            // stored spec needs migrating.
            let raw = if p.name == "interval" {
                &crate::spec::normalize_interval(raw)
            } else {
                raw
            };
            args.insert_raw(p.name, p.param_type, raw)
                .map_err(|e| crate::Error::Other(format!("arg {}: {}", p.name, e)))?;
        } else if p.required {
            return Err(crate::Error::Other(format!(
                "missing required arg '{}' for endpoint '{}'",
                p.name, spec.endpoint
            )));
        }
    }
    invoke_endpoint(client.market_data(), &spec.endpoint, &args)
        .await
        .map_err(|e| crate::Error::Other(format!("invoke {}: {:?}", spec.endpoint, e)))
}

/// Convert any `EndpointOutput` variant that carries tick data into a
/// `RecordBatch`. `StringList` and similar non-tick outputs return `None`.
///
/// Every arm goes through `Ticks::to_arrow`, which projects to the columns
/// the response actually carried. The slice-level `TicksArrowExt::to_arrow`
/// emits the tick type's full schema instead, so an equity response would
/// gain always-null contract-identity columns and a gRPC trade response
/// would gain four always-zero flag columns that the wire never sent.
pub fn arrow_from_output(out: &EndpointOutput) -> Option<RecordBatch> {
    use EndpointOutput::*;
    match out {
        StringList(_) => None,
        EodTicks(v) => v.to_arrow().ok(),
        OhlcTicks(v) => v.to_arrow().ok(),
        TradeTicks(v) => v.to_arrow().ok(),
        QuoteTicks(v) => v.to_arrow().ok(),
        TradeQuoteTicks(v) => v.to_arrow().ok(),
        OpenInterestTicks(v) => v.to_arrow().ok(),
        MarketValueTicks(v) => v.to_arrow().ok(),
        GreeksAllTicks(v) => v.to_arrow().ok(),
        GreeksEodTicks(v) => v.to_arrow().ok(),
        GreeksFirstOrderTicks(v) => v.to_arrow().ok(),
        GreeksSecondOrderTicks(v) => v.to_arrow().ok(),
        GreeksThirdOrderTicks(v) => v.to_arrow().ok(),
        TradeGreeksAllTicks(v) => v.to_arrow().ok(),
        TradeGreeksFirstOrderTicks(v) => v.to_arrow().ok(),
        TradeGreeksSecondOrderTicks(v) => v.to_arrow().ok(),
        TradeGreeksThirdOrderTicks(v) => v.to_arrow().ok(),
        TradeGreeksImpliedVolatilityTicks(v) => v.to_arrow().ok(),
        IvTicks(v) => v.to_arrow().ok(),
        PriceTicks(v) => v.to_arrow().ok(),
        IndexPriceAtTimeTicks(v) => v.to_arrow().ok(),
        CalendarDays(v) => v.to_arrow().ok(),
        InterestRateTicks(v) => v.to_arrow().ok(),
        OptionContracts(v) => v.to_arrow().ok(),
    }
}

//! Subscription-tier gating.
//!
//! ThetaData partitions historical access into four tiers per asset class
//! (Free / Value / Standard / Pro). The Nexus auth response carries the
//! customer's tier per asset class; `thetadatadx` exposes it as
//! `SubscriptionInfo { stock, options }`. We map endpoint category +
//! subcategory to a conservative minimum tier and expose a
//! ranked-comparison gate plus a stable upgrade URL the UI links to when
//! the user is below the bar.
//!
//! The gating is advisory — the gRPC server is the source of truth and
//! returns `PermissionDenied` for under-entitled calls. The UI uses this
//! module to grey out cards before a user wastes time queueing a request
//! that will fail.
//!
//! The mapping table reflects ThetaData's published tier matrix at the
//! time of writing; if the vendor moves an endpoint between tiers we
//! adjust this table, not the calling code.
//!
//! Banned vocabulary: "Manager", "Helper", "Oracle". We say `Tier`,
//! `TierGate`, `UserTiers` — tracking ThetaData's own SDK terminology.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::registry::EndpointInfo;

/// Public upgrade URL surfaced to the UI when a dataset is gated.
pub const UPGRADE_URL: &str = "https://thetadata.net/pricing";

/// Customer subscription tier, ordered by `rank()`. `Unknown` ranks
/// below `Free` so any "tier needed" check for a real tier fails when
/// auth didn't return a tier byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
// Variants serialize PascalCase ("Free" | "Value" | …) to match the
// FE `TierName` literal union. Earlier rename_all="lowercase" caused
// silent breakage in TierVerdict + CatalogueEntry.min_tier — the FE
// indexed `TIER_RANK[verdict.required]` by capitalized and got
// `undefined`, so tier-rank sort + meets-check both no-op'd.
pub enum Tier {
    Unknown,
    Free,
    Value,
    Standard,
    Pro,
}

impl Tier {
    /// Higher = more access. Used by `meets`.
    pub const fn rank(self) -> i32 {
        match self {
            Tier::Unknown => -1,
            Tier::Free => 0,
            Tier::Value => 1,
            Tier::Standard => 2,
            Tier::Pro => 3,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Unknown => "Unknown",
            Tier::Free => "Free",
            Tier::Value => "Value",
            Tier::Standard => "Standard",
            Tier::Pro => "Pro",
        }
    }

    /// Parse the wire/label string emitted by `thetadatadx` (case-insensitive).
    pub fn from_label(s: &str) -> Tier {
        match s.trim().to_ascii_lowercase().as_str() {
            "free" | "0" => Tier::Free,
            "value" | "1" => Tier::Value,
            "standard" | "2" => Tier::Standard,
            "pro" | "professional" | "3" => Tier::Pro,
            _ => Tier::Unknown,
        }
    }

    /// True when `self` grants at least the access of `required`.
    pub const fn meets(self, required: Tier) -> bool {
        self.rank() >= required.rank()
    }

    /// Max in-flight requests the ThetaData FPSS server will accept for
    /// this tier. The terminal documents the cap as `2^subscription_tier`
    /// per asset class: Free=1, Value=2, Standard=4, Pro=8. We mirror
    /// that 1:1 — going over makes the server queue requests internally
    /// or drop them with a 429, both of which look like flaky downloads
    /// on the client. `Unknown` falls back to 1 so a missing tier
    /// (pre-connect, or a class the upstream SDK hasn't surfaced yet)
    /// gracefully degrades to serial rather than spinning up phantom
    /// workers that all queue behind one another.
    pub const fn workers(self) -> usize {
        match self {
            Tier::Unknown | Tier::Free => 1,
            Tier::Value => 2,
            Tier::Standard => 4,
            Tier::Pro => 8,
        }
    }
}

/// Which Nexus asset-class pool a tick belongs to. Each class has its own
/// server-side concurrency budget and its own per-class subscription
/// tier, so the client must mirror that split: a Pro Options subscriber
/// with a Standard Stocks subscription gets 8 option workers + 4 stock
/// workers running simultaneously, not min/max/sum of the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    Stock,
    Option,
    Index,
    Rate,
}

impl AssetClass {
    pub fn as_str(self) -> &'static str {
        match self {
            AssetClass::Stock => "stock",
            AssetClass::Option => "option",
            AssetClass::Index => "index",
            AssetClass::Rate => "rate",
        }
    }
}

/// User's tier per asset class. ThetaData's Nexus auth response carries
/// four independent subscription bytes (`stock_subscription`,
/// `options_subscription`, `indices_subscription`,
/// `interest_rate_subscription`). `thetadatadx` v10 only exposes the
/// first two on `SubscriptionInfo`; the latter two are filled with
/// `Tier::Unknown` until the upstream SDK surfaces accessor methods.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UserTiers {
    pub stock: Tier,
    pub options: Tier,
    pub indices: Tier,
    pub interest_rate: Tier,
}

impl Default for UserTiers {
    fn default() -> Self {
        Self {
            stock: Tier::Unknown,
            options: Tier::Unknown,
            indices: Tier::Unknown,
            interest_rate: Tier::Unknown,
        }
    }
}

impl UserTiers {
    /// Tier the user holds for `class`. Drives per-class worker
    /// concurrency (see `Tier::workers`).
    pub fn for_class(&self, class: AssetClass) -> Tier {
        match class {
            AssetClass::Stock => self.stock,
            AssetClass::Option => self.options,
            AssetClass::Index => self.indices,
            AssetClass::Rate => self.interest_rate,
        }
    }

    /// In-flight request budget granted by `class`'s tier.
    pub fn workers_for(&self, class: AssetClass) -> usize {
        self.for_class(class).workers()
    }
}

// Generated at build time from `spec/openapiv3.yaml` via `build.rs`.
// Provides:
//   - `min_tier_for_endpoint_generated(op) -> Option<Tier>`
//   - `ENDPOINT_META_TABLE: &[(op, summary, description, tag, tier)]`
//   - `ENDPOINT_META_COUNT: usize`
include!(concat!(env!("OUT_DIR"), "/yaml_metadata_generated.rs"));

/// Process-wide override populated by `install_remote_table` when the
/// app fetches a fresh `openapiv3.yaml` from `docs.thetadata.us` at
/// runtime. When present, takes priority over the build-time table —
/// so a desktop install picks up tier changes ThetaData publishes
/// after the binary was built.
static REMOTE_TABLE: std::sync::OnceLock<std::sync::RwLock<Option<HashMap<String, Tier>>>> =
    std::sync::OnceLock::new();

fn remote_lookup(operation_id: &str) -> Option<Tier> {
    let cell = REMOTE_TABLE.get()?;
    let guard = cell.read().ok()?;
    let map = guard.as_ref()?;
    map.get(operation_id).copied()
}

/// Install (or replace) the runtime tier table with a freshly-fetched
/// map from the upstream OpenAPI spec. Subsequent
/// `min_tier_for_endpoint` calls consult this map first, falling back
/// to the build-time table only for endpoints not present in the
/// remote map (e.g. brand-new ops we don't yet know about locally).
pub fn install_remote_table(map: HashMap<String, Tier>) {
    let cell = REMOTE_TABLE.get_or_init(|| std::sync::RwLock::new(None));
    if let Ok(mut guard) = cell.write() {
        *guard = Some(map);
    }
}

/// Required tier for a given operationId. Looks up first in the
/// runtime-installed remote table (if any), then the build-time
/// generated table baked from the vendored yaml. Returns `None` only
/// when the endpoint is unknown to both.
pub fn min_tier_for_endpoint(operation_id: &str) -> Option<Tier> {
    if let Some(t) = remote_lookup(operation_id) {
        return Some(t);
    }
    min_tier_for_endpoint_generated(operation_id)
}

/// Canonical upstream OpenAPI spec URL — re-fetched at runtime so the
/// app picks up tier changes ThetaData publishes after the binary
/// was built.
pub const OPENAPI_SPEC_URL: &str = "https://docs.thetadata.us/openapiv3.yaml";

/// Pull the latest `openapiv3.yaml` from `OPENAPI_SPEC_URL`, parse
/// every `x-min-subscription` field into a `(operationId -> Tier)`
/// map, and install the result via `install_remote_table`. Returns
/// the number of endpoints in the freshly-installed table.
///
/// Failures (network down, malformed yaml) leave the previous table
/// in place — callers should log warnings, not panic. The build-time
/// table from `spec/openapiv3.yaml` continues to serve as the offline
/// fallback regardless.
pub async fn fetch_and_install_remote() -> crate::Result<usize> {
    let yaml = reqwest::get(OPENAPI_SPEC_URL)
        .await
        .map_err(|e| crate::Error::Other(format!("openapi fetch: {e}")))?
        .error_for_status()
        .map_err(|e| crate::Error::Other(format!("openapi http: {e}")))?
        .text()
        .await
        .map_err(|e| crate::Error::Other(format!("openapi body: {e}")))?;
    let (tier_map, meta_map) = parse_full_metadata(&yaml)
        .map_err(|e| crate::Error::Other(format!("openapi parse: {e}")))?;
    let n = tier_map.len();
    install_remote_table(tier_map);
    crate::yaml_meta::install_runtime_meta(meta_map);
    Ok(n)
}

/// Walk the OpenAPI document and pull both the tier table AND the
/// per-endpoint metadata (summary, description, tag) in one pass.
/// State machine over indent levels — see comment in `build.rs` for
/// why we plain-text-scan rather than use a YAML parser.
#[allow(clippy::type_complexity)]
fn parse_full_metadata(
    yaml: &str,
) -> Result<
    (
        HashMap<String, Tier>,
        HashMap<String, crate::yaml_meta::EndpointMeta>,
    ),
    String,
> {
    let mut tiers: HashMap<String, Tier> = HashMap::new();
    let mut metas: HashMap<String, crate::yaml_meta::EndpointMeta> = HashMap::new();

    let mut in_paths = false;
    let mut current: Option<String> = None;
    let mut in_get_block = false;
    let mut collecting_description: Option<usize> = None;
    let mut description_buf: Vec<String> = Vec::new();
    let mut in_tags_block = false;
    let mut waiting_first_tag = false;

    for raw_line in yaml.lines() {
        if !raw_line.starts_with(' ') && raw_line.trim_end().ends_with(':') {
            in_paths = raw_line.trim_start().starts_with("paths:");
            current = None;
            in_get_block = false;
            collecting_description = None;
            in_tags_block = false;
            continue;
        }
        if !in_paths {
            continue;
        }

        let trimmed = raw_line.trim_start();
        let indent = raw_line.len() - trimmed.len();

        if let Some(base_indent) = collecting_description {
            if trimmed.is_empty() {
                description_buf.push(String::new());
                continue;
            }
            if indent > base_indent {
                description_buf.push(trimmed.trim_end().to_string());
                continue;
            }
            if let Some(op) = current.as_ref() {
                let m = metas.entry(op.clone()).or_insert_with(|| empty_meta(op));
                if m.description.is_empty() {
                    m.description = description_buf.join(" ").trim().to_string();
                }
            }
            description_buf.clear();
            collecting_description = None;
        }

        if indent == 2 && trimmed.starts_with('/') && raw_line.trim_end().ends_with(':') {
            let raw_path = trimmed.trim_end().trim_end_matches(':');
            let key = path_to_op_id(raw_path);
            current = if key.is_empty() { None } else { Some(key) };
            in_get_block = false;
            in_tags_block = false;
            continue;
        }

        let Some(op) = current.as_ref().cloned() else {
            continue;
        };

        if indent == 4 {
            if let Some(rest) = trimmed.strip_prefix("x-min-subscription:") {
                let raw_tier = rest.trim();
                let tier = match raw_tier.to_ascii_lowercase().as_str() {
                    "free" => Some(Tier::Free),
                    "value" => Some(Tier::Value),
                    "standard" => Some(Tier::Standard),
                    "pro" | "professional" => Some(Tier::Pro),
                    _ => None,
                };
                if let Some(t) = tier {
                    tiers.insert(op.clone(), t);
                    metas
                        .entry(op.clone())
                        .or_insert_with(|| empty_meta(&op))
                        .min_tier = Some(t);
                }
                continue;
            }
            if trimmed.starts_with("get:") {
                in_get_block = true;
                in_tags_block = false;
                continue;
            }
            in_get_block = false;
            in_tags_block = false;
            continue;
        }

        if in_get_block && indent == 6 {
            in_tags_block = false;
            if let Some(rest) = trimmed.strip_prefix("summary:") {
                let val = rest.trim();
                if !val.is_empty() {
                    let m = metas.entry(op.clone()).or_insert_with(|| empty_meta(&op));
                    if m.summary.is_empty() {
                        m.summary = unquote(val).to_string();
                    }
                }
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("description:") {
                let val = rest.trim();
                if val == "|" || val == "|-" || val == ">" || val == ">-" {
                    collecting_description = Some(indent);
                    description_buf.clear();
                } else if !val.is_empty() {
                    let m = metas.entry(op.clone()).or_insert_with(|| empty_meta(&op));
                    if m.description.is_empty() {
                        m.description = unquote(val).to_string();
                    }
                }
                continue;
            }
            if trimmed.starts_with("tags:") {
                in_tags_block = true;
                waiting_first_tag = true;
                continue;
            }
            continue;
        }

        if in_tags_block && indent == 8 && trimmed.starts_with("- ") && waiting_first_tag {
            let tag = trimmed[2..].trim();
            let m = metas.entry(op.clone()).or_insert_with(|| empty_meta(&op));
            if m.tag.is_empty() {
                m.tag = unquote(tag).to_string();
            }
            waiting_first_tag = false;
            in_tags_block = false;
        }
    }

    metas.retain(|_, m| m.min_tier.is_some());
    Ok((tiers, metas))
}

fn empty_meta(op: &str) -> crate::yaml_meta::EndpointMeta {
    crate::yaml_meta::EndpointMeta {
        operation_id: op.to_string(),
        summary: String::new(),
        description: String::new(),
        tag: String::new(),
        min_tier: None,
    }
}

fn unquote(s: &str) -> &str {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn path_to_op_id(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        if seg.is_empty() || seg.starts_with('{') {
            continue;
        }
        parts.push(seg);
    }
    parts.join("_")
}

/// Required tier for a given (category, subcategory). Heuristic
/// fallback when the caller doesn't have the operationId at hand —
/// PREFER `min_tier_for_endpoint` when you do, since the authoritative
/// table is more accurate (e.g. `option_history_trade` is actually
/// Standard, not Pro).
///
/// | category | subcategory          | min tier  |
/// |----------|----------------------|-----------|
/// | stock    | history_eod          | Free      |
/// | stock    | history              | Value     |
/// | stock    | snapshot             | Value     |
/// | stock    | at_time              | Standard  |
/// | option   | history_eod          | Free      |
/// | option   | history              | Value     |
/// | option   | snapshot             | Value     |
/// | option   | snapshot_greeks      | Standard  |
/// | option   | history_greeks       | Standard  |
/// | option   | at_time              | Standard  |
/// | option   | list                 | Free      |
/// | index    | history              | Standard  |
/// | rate     | *                    | Value     |
/// | calendar | *                    | Free      |
/// | *        | list                 | Free      |
pub fn min_tier_for(category: &str, subcategory: &str) -> Tier {
    let cat = category.trim().to_ascii_lowercase();
    let sub = subcategory.trim().to_ascii_lowercase();
    if sub == "list" {
        return Tier::Free;
    }
    match cat.as_str() {
        "stock" => match sub.as_str() {
            "history_eod" => Tier::Free,
            "snapshot" => Tier::Value,
            "history" => Tier::Value,
            "at_time" => Tier::Standard,
            _ => Tier::Value,
        },
        "option" => match sub.as_str() {
            "history_eod" => Tier::Free,
            "snapshot" => Tier::Value,
            "history" => Tier::Value,
            "snapshot_greeks" | "history_greeks" => Tier::Standard,
            "at_time" => Tier::Standard,
            _ => Tier::Value,
        },
        "index" => match sub.as_str() {
            "history" | "snapshot" => Tier::Standard,
            _ => Tier::Free,
        },
        "rate" => Tier::Value,
        "calendar" => Tier::Free,
        _ => Tier::Free,
    }
}

/// Which asset-class tier governs a given endpoint category.
/// ThetaData publishes four independent subscription bytes — when the
/// upstream SDK exposes the indices/rate accessors, this routes
/// through them. `calendar` endpoints have no tier requirement so we
/// fall back to the most-permissive (stock) tier.
pub fn governing_tier(category: &str, user: &UserTiers) -> Tier {
    match category.trim().to_ascii_lowercase().as_str() {
        "option" => user.options,
        "index" => user.indices,
        "rate" => user.interest_rate,
        // stock + calendar gate against the stock tier (calendar has
        // no real requirement; routing it here keeps the Free-or-above
        // check predictable).
        _ => user.stock,
    }
}

/// Result of evaluating one endpoint against a user's tiers.
#[derive(Debug, Clone, Serialize)]
pub struct TierVerdict {
    pub endpoint: String,
    pub category: String,
    pub subcategory: String,
    pub required: Tier,
    pub user: Tier,
    pub allowed: bool,
}

pub fn evaluate(info: &EndpointInfo, user: &UserTiers) -> TierVerdict {
    // Authoritative per-endpoint table beats the heuristic when we
    // have an operationId match. Falls back to the (category,
    // subcategory) heuristic for endpoints not yet in the table.
    let required = min_tier_for_endpoint(&info.name)
        .unwrap_or_else(|| min_tier_for(&info.category, &info.subcategory));
    let user_tier = governing_tier(&info.category, user);
    TierVerdict {
        endpoint: info.name.clone(),
        category: info.category.clone(),
        subcategory: info.subcategory.clone(),
        required,
        user: user_tier,
        allowed: user_tier.meets(required),
    }
}

/// Detect whether an `Error` came from the gRPC server with a
/// PermissionDenied code, which is how the upstream signals "tier
/// insufficient". Used by the UI/error toaster to route to an upgrade
/// CTA rather than a generic "operation failed" message.
pub fn is_tier_denied(err: &crate::Error) -> bool {
    let crate::Error::Theta(theta) = err else {
        return false;
    };
    let thetadatadx::Error::Grpc { kind, .. } = theta else {
        return false;
    };
    matches!(kind, thetadatadx::error::GrpcStatusKind::PermissionDenied)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str, cat: &str, sub: &str) -> EndpointInfo {
        EndpointInfo {
            name: name.into(),
            description: String::new(),
            category: cat.into(),
            subcategory: sub.into(),
            rest_path: String::new(),
            returns: String::new(),
            params: vec![],
        }
    }

    #[test]
    fn tier_ranks_strict_monotonic() {
        let order = [
            Tier::Unknown,
            Tier::Free,
            Tier::Value,
            Tier::Standard,
            Tier::Pro,
        ];
        for w in order.windows(2) {
            assert!(w[0].rank() < w[1].rank(), "{:?} >= {:?}", w[0], w[1]);
        }
    }

    #[test]
    fn meets_higher_or_equal() {
        assert!(Tier::Pro.meets(Tier::Standard));
        assert!(Tier::Standard.meets(Tier::Standard));
        assert!(!Tier::Value.meets(Tier::Standard));
        assert!(!Tier::Unknown.meets(Tier::Free));
    }

    #[test]
    fn from_label_handles_all_variants() {
        assert_eq!(Tier::from_label("Free"), Tier::Free);
        assert_eq!(Tier::from_label("VALUE"), Tier::Value);
        assert_eq!(Tier::from_label("standard"), Tier::Standard);
        assert_eq!(Tier::from_label("Pro"), Tier::Pro);
        assert_eq!(Tier::from_label("Professional"), Tier::Pro);
        assert_eq!(Tier::from_label("3"), Tier::Pro);
        assert_eq!(Tier::from_label("garbage"), Tier::Unknown);
        assert_eq!(Tier::from_label(""), Tier::Unknown);
    }

    #[test]
    fn list_endpoints_always_free() {
        assert_eq!(min_tier_for("stock", "list"), Tier::Free);
        assert_eq!(min_tier_for("option", "list"), Tier::Free);
        assert_eq!(min_tier_for("index", "list"), Tier::Free);
    }

    #[test]
    fn authoritative_table_beats_heuristic() {
        // Per OpenAPI x-min-subscription, option_history_trade is
        // Standard, not Pro. The endpoint-name lookup must win over
        // the (category, subcategory) heuristic.
        assert_eq!(
            min_tier_for_endpoint("option_history_trade"),
            Some(Tier::Standard)
        );
        assert_eq!(
            min_tier_for_endpoint("option_history_quote"),
            Some(Tier::Value)
        );
        assert_eq!(
            min_tier_for_endpoint("option_history_greeks_all"),
            Some(Tier::Pro)
        );
        assert_eq!(min_tier_for_endpoint("stock_history_eod"), Some(Tier::Free));
        // Unknown ops fall through to the heuristic.
        assert_eq!(min_tier_for_endpoint("not_a_real_endpoint"), None);
    }

    #[test]
    fn evaluate_blocks_when_under_tier() {
        // option_history_greeks_all needs Pro. User on Standard
        // options is below it.
        let user = UserTiers {
            stock: Tier::Standard,
            options: Tier::Standard,
            indices: Tier::Unknown,
            interest_rate: Tier::Unknown,
        };
        let v = evaluate(
            &info("option_history_greeks_all", "option", "history_greeks"),
            &user,
        );
        assert!(!v.allowed);
        assert_eq!(v.required, Tier::Pro);
        assert_eq!(v.user, Tier::Standard);
    }

    #[test]
    fn evaluate_allows_when_over_tier() {
        let user = UserTiers {
            stock: Tier::Pro,
            options: Tier::Pro,
            indices: Tier::Pro,
            interest_rate: Tier::Pro,
        };
        let v = evaluate(&info("stock_history_eod", "stock", "history_eod"), &user);
        assert!(v.allowed);
    }

    #[test]
    fn governing_tier_routes_per_asset_class() {
        let user = UserTiers {
            stock: Tier::Free,
            options: Tier::Pro,
            indices: Tier::Standard,
            interest_rate: Tier::Value,
        };
        assert_eq!(governing_tier("stock", &user), Tier::Free);
        assert_eq!(governing_tier("option", &user), Tier::Pro);
        assert_eq!(governing_tier("index", &user), Tier::Standard);
        assert_eq!(governing_tier("rate", &user), Tier::Value);
        // calendar has no tier — falls back to stock so the test
        // proves we route, not panic.
        assert_eq!(governing_tier("calendar", &user), Tier::Free);
    }

    #[test]
    fn upgrade_url_is_https() {
        assert!(UPGRADE_URL.starts_with("https://"));
    }
}

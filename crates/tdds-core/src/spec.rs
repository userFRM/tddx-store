//! What to download. A `DataSpec` is one (dataset x symbol x date) work
//! unit; an `EndpointSpec` is the generic registry call it lowers onto.
//!
//! Both name a dataset the same way the catalogue does: by its registry
//! endpoint (`stock_history_trade`, `option_history_greeks_all`, ...).
//! There is deliberately no second, shorter list of "core" kinds — the
//! catalogue offered every registry dataset while the queue accepted
//! seven names, and
//! every dataset outside that overlap failed to queue at all.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// The dataset a task pulls, named by its registry endpoint.
///
/// Construct with [`DataKind::parse`], which accepts a name only if the
/// registry knows it. That keeps an unknown dataset out of the queue at
/// the boundary instead of failing later against the server.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DataKind(String);

/// Dataset names this app used before the queue and the catalogue
/// shared one vocabulary. Queue rows, schedules and on-disk directories
/// written by those builds still carry them, so they keep resolving.
const LEGACY_ALIASES: &[(&str, &str)] = &[
    ("stock_trade", "stock_history_trade"),
    ("stock_quote", "stock_history_quote"),
    ("stock_trade_quote", "stock_history_trade_quote"),
    ("option_trade", "option_history_trade"),
    ("option_quote", "option_history_quote"),
    ("option_trade_quote", "option_history_trade_quote"),
    ("option_oi", "option_history_open_interest"),
];

impl DataKind {
    /// Resolve a dataset name against the registry. Legacy names are
    /// translated first; anything the registry does not know returns
    /// `None`.
    pub fn parse(s: &str) -> Option<Self> {
        let name = LEGACY_ALIASES
            .iter()
            .find(|(legacy, _)| *legacy == s)
            .map_or(s, |(_, current)| *current);
        thetadatadx::find(name).map(|meta| Self(meta.name.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The registry endpoint this dataset dispatches to. Same string —
    /// the dataset *is* the endpoint.
    pub fn endpoint(&self) -> &str {
        &self.0
    }

    fn meta(&self) -> Option<&'static thetadatadx::EndpointMeta> {
        thetadatadx::find(&self.0)
    }

    /// Whether this endpoint declares `param`. Read from the registry so
    /// a vendor-side parameter change needs no edit here.
    pub fn takes_param(&self, param: &str) -> bool {
        self.meta()
            .is_some_and(|m| m.params.iter().any(|p| p.name == param))
    }

    pub fn takes_interval(&self) -> bool {
        self.takes_param("interval")
    }

    pub fn is_option(&self) -> bool {
        matches!(self.asset_class(), crate::tier::AssetClass::Option)
    }

    /// Which asset class gates this dataset, from the registry
    /// category. Drives tier gating, not concurrency.
    pub fn asset_class(&self) -> crate::tier::AssetClass {
        match self.meta().map(|m| m.category).unwrap_or("") {
            "option" => crate::tier::AssetClass::Option,
            "index" => crate::tier::AssetClass::Index,
            "rate" => crate::tier::AssetClass::Rate,
            _ => crate::tier::AssetClass::Stock,
        }
    }
}

impl std::fmt::Display for DataKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One unit of work: pull `kind` for `symbol` on that calendar `date`.
/// The contract filters and `interval` are carried for every dataset and
/// applied only where the endpoint declares the matching parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSpec {
    pub kind: DataKind,
    pub symbol: String,
    pub date: NaiveDate,
    /// `"tick"` = every update; `"1s"` and up are sampled. Applied only
    /// to endpoints that declare an `interval` parameter.
    #[serde(default)]
    pub interval: Option<String>,
    /// `*` (default) = every expiration on this date.
    #[serde(default = "default_expiration")]
    pub expiration: String,
    /// `*` = every strike.
    #[serde(default = "default_strike")]
    pub strike: String,
    /// `both` = calls and puts.
    #[serde(default = "default_right")]
    pub right: String,
    /// Post-decode transforms applied before write (rename / drop / scale).
    #[serde(default)]
    pub transforms: crate::Transforms,
}

fn default_expiration() -> String {
    "*".into()
}
fn default_strike() -> String {
    "*".into()
}
fn default_right() -> String {
    "both".into()
}

impl DataSpec {
    pub fn ymd(&self) -> String {
        self.date.format("%Y%m%d").to_string()
    }

    /// Stable file stem used everywhere on disk.
    pub fn file_stem(&self) -> String {
        format!(
            "{}_{}_{}",
            self.symbol.to_lowercase(),
            self.kind.as_str(),
            self.ymd()
        )
    }

    /// Lower this work unit onto the generic registry spec, filling only
    /// the parameters the endpoint actually declares.
    ///
    /// Queue tasks and the endpoint browser therefore share one
    /// dispatcher, one argument-validation path, and one Arrow
    /// projection. Driving the fill off the registry rather than off a
    /// per-dataset match means an endpoint that grows a parameter picks
    /// it up with no edit here, and one that lacks `strike` never
    /// receives it.
    pub fn to_endpoint_spec(&self) -> EndpointSpec {
        let mut spec = EndpointSpec::new(self.kind.endpoint());
        let Some(meta) = thetadatadx::find(self.kind.endpoint()) else {
            return spec;
        };
        for p in meta.params {
            let value = match p.name {
                "symbol" => Some(self.symbol.clone()),
                "date" => Some(self.ymd()),
                "expiration" => Some(self.expiration.clone()),
                "strike" => Some(self.strike.clone()),
                "right" => Some(self.right.clone()),
                "interval" => self.interval.clone(),
                _ => None,
            };
            if let Some(v) = value {
                spec = spec.arg(p.name, v);
            }
        }
        spec
    }
}

/// One selectable sampling interval: the wire value the endpoint takes
/// and the label the UI shows for it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IntervalOption {
    pub id: &'static str,
    pub label: &'static str,
}

/// Every interval ThetaData accepts, in ascending order. These are wire
/// values, not free text — the endpoint rejects anything outside this
/// set, and `intervals_are_accepted_by_the_sdk` proves each one parses.
/// The UI renders this list rather than keeping its own, so a vendor
/// change lands in one place.
pub const INTERVALS: &[IntervalOption] = &[
    IntervalOption {
        id: "tick",
        label: "Tick by tick",
    },
    IntervalOption {
        id: "10ms",
        label: "10 ms",
    },
    IntervalOption {
        id: "100ms",
        label: "100 ms",
    },
    IntervalOption {
        id: "500ms",
        label: "500 ms",
    },
    IntervalOption {
        id: "1s",
        label: "1 second",
    },
    IntervalOption {
        id: "5s",
        label: "5 seconds",
    },
    IntervalOption {
        id: "10s",
        label: "10 seconds",
    },
    IntervalOption {
        id: "15s",
        label: "15 seconds",
    },
    IntervalOption {
        id: "30s",
        label: "30 seconds",
    },
    IntervalOption {
        id: "1m",
        label: "1 minute",
    },
    IntervalOption {
        id: "5m",
        label: "5 minutes",
    },
    IntervalOption {
        id: "10m",
        label: "10 minutes",
    },
    IntervalOption {
        id: "15m",
        label: "15 minutes",
    },
    IntervalOption {
        id: "30m",
        label: "30 minutes",
    },
    IntervalOption {
        id: "1h",
        label: "1 hour",
    },
];

/// Map legacy interval spellings onto the wire values ThetaData
/// accepts. The v2 API took a millisecond integer where `0` meant
/// "every update"; v3 names that `tick` and rejects everything outside
/// the named set. Queue rows, saved searches and schedules written
/// before the rename still carry the old spellings, so they are
/// translated on the way out rather than migrated in place.
pub fn normalize_interval(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(named) = match raw {
        "0" => Some("tick"),
        "10" => Some("10ms"),
        "100" => Some("100ms"),
        "500" => Some("500ms"),
        "1000" => Some("1s"),
        "5000" => Some("5s"),
        "10000" => Some("10s"),
        "15000" => Some("15s"),
        "30000" => Some("30s"),
        "60000" | "60s" => Some("1m"),
        "300000" | "300s" => Some("5m"),
        "600000" | "600s" => Some("10m"),
        "900000" | "900s" => Some("15m"),
        "1800000" | "1800s" => Some("30m"),
        "3600000" | "3600s" => Some("1h"),
        _ => None,
    } {
        return named.to_string();
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The picker may only offer values the endpoint will accept. If the
    /// vendor renames an interval, this fails here rather than on a
    /// user's download.
    #[test]
    fn intervals_are_accepted_by_the_sdk() {
        for opt in INTERVALS {
            assert!(
                thetadatadx::Interval::try_from(opt.id).is_ok(),
                "interval `{}` is not accepted by the SDK",
                opt.id
            );
        }
    }

    /// Every legacy spelling must land on a value the SDK accepts —
    /// otherwise a queue row written by an older build fails at run time
    /// with an opaque parameter error.
    #[test]
    fn legacy_interval_spellings_normalize_to_accepted_values() {
        for legacy in [
            "0", "10", "100", "500", "1000", "5000", "10000", "15000", "30000", "60000", "60s",
            "300000", "300s", "600000", "600s", "900000", "900s", "1800000", "1800s", "3600000",
            "3600s",
        ] {
            let normalized = normalize_interval(legacy);
            assert!(
                thetadatadx::Interval::try_from(normalized.as_str()).is_ok(),
                "legacy interval `{legacy}` normalized to `{normalized}`, which the SDK rejects",
            );
        }
    }

    /// A value already in wire form survives normalization untouched.
    #[test]
    fn wire_intervals_pass_through_normalization() {
        for opt in INTERVALS {
            assert_eq!(normalize_interval(opt.id), opt.id);
        }
    }
}

/// Generic endpoint spec — covers every endpoint in the SDK registry via the
/// registry-driven `invoke_endpoint` dispatcher. Both required (positional
/// upstream) and optional (fluent .setter) params are normalized into one
/// flat string-keyed bag; the dispatcher resolves type per endpoint
/// metadata in `thetadatadx::find(name).params`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointSpec {
    /// MddsClient method name, e.g. `"option_history_trade_quote"`.
    pub endpoint: String,
    /// All params (required + optional) keyed by name. Strings only — the
    /// dispatcher casts to the expected type using the registry's
    /// `ParamType`. For example `interval: "0"` is coerced for quote
    /// endpoints, `max_dte: "30"` parses to i32, etc.
    #[serde(default)]
    pub args: BTreeMap<String, String>,
    /// Per-call deadline (millis). 0 = no deadline.
    #[serde(default)]
    pub timeout_ms: u64,
}

impl EndpointSpec {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            args: BTreeMap::new(),
            timeout_ms: 0,
        }
    }

    pub fn arg(mut self, key: &str, value: impl Into<String>) -> Self {
        self.args.insert(key.to_string(), value.into());
        self
    }

    pub fn with_timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    /// Stable file stem: `<endpoint>__<sorted_arg_kvs>`. Date and symbol are
    /// part of `args`, so this is unique per call.
    pub fn file_stem(&self) -> String {
        let kvs: Vec<String> = self
            .args
            .iter()
            .map(|(k, v)| format!("{k}-{}", v.replace(['/', '\\', ' ', '*'], "_")))
            .collect();
        format!("{}__{}", self.endpoint, kvs.join("_"))
    }
}

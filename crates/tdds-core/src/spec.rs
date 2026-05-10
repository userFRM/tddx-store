//! What to download. A `DataSpec` is one (kind × symbol × date) work unit.
//!
//! There are two layers:
//!
//! - `DataKind` — the seven "core" tick endpoints we keep first-class
//!   for legacy code paths and the simple `add` CLI. Hardcoded match.
//! - `EndpointSpec` — generic registry-backed spec covering every one of
//!   thetadatadx's 61 endpoints (history / snapshot / list / at_time /
//!   greeks). Positional required params + fluent optionals are both
//!   handled by `thetadatadx::invoke_endpoint`.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// All ThetaData historical endpoints we expose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataKind {
    StockTrade,
    StockQuote,
    StockTradeQuote,
    OptionTrade,
    OptionQuote,
    OptionTradeQuote,
    OptionOpenInterest,
}

impl DataKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DataKind::StockTrade => "stock_trade",
            DataKind::StockQuote => "stock_quote",
            DataKind::StockTradeQuote => "stock_trade_quote",
            DataKind::OptionTrade => "option_trade",
            DataKind::OptionQuote => "option_quote",
            DataKind::OptionTradeQuote => "option_trade_quote",
            DataKind::OptionOpenInterest => "option_oi",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "stock_trade" => DataKind::StockTrade,
            "stock_quote" => DataKind::StockQuote,
            "stock_trade_quote" => DataKind::StockTradeQuote,
            "option_trade" => DataKind::OptionTrade,
            "option_quote" => DataKind::OptionQuote,
            "option_trade_quote" => DataKind::OptionTradeQuote,
            "option_oi" => DataKind::OptionOpenInterest,
            _ => return None,
        })
    }

    pub fn all() -> &'static [DataKind] {
        &[
            DataKind::StockTrade,
            DataKind::StockQuote,
            DataKind::StockTradeQuote,
            DataKind::OptionTrade,
            DataKind::OptionQuote,
            DataKind::OptionTradeQuote,
            DataKind::OptionOpenInterest,
        ]
    }

    pub fn is_option(self) -> bool {
        matches!(
            self,
            DataKind::OptionTrade
                | DataKind::OptionQuote
                | DataKind::OptionTradeQuote
                | DataKind::OptionOpenInterest
        )
    }

    /// Which ThetaData asset-class pool this kind draws from. Drives
    /// worker scheduling — each class has its own concurrency budget
    /// sized by the user's per-class subscription tier.
    pub fn asset_class(self) -> crate::tier::AssetClass {
        if self.is_option() {
            crate::tier::AssetClass::Option
        } else {
            crate::tier::AssetClass::Stock
        }
    }
}

/// One unit of work: pull `kind` for `symbol` on calendar `date`. The optional
/// `interval` and `expiration_filter` are used by quote / option calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSpec {
    pub kind: DataKind,
    pub symbol: String,
    pub date: NaiveDate,
    /// `interval="0"` = every NBBO update; `"1s"` = sampled. Only meaningful
    /// for `*Quote` kinds.
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
}

/// Generic endpoint spec — covers all 61 thetadatadx endpoints via the
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

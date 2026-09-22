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
    /// The last date of a range pull, when the endpoint takes a range
    /// rather than a single day.
    ///
    /// Endpoints split into two shapes: some declare `date` and are
    /// fanned out one task per trading day, others declare
    /// `start_date`/`end_date` and answer a whole window in one call.
    /// `date` is the start in both cases; this is the end, and it is
    /// `None` for the per-day shape.
    #[serde(default)]
    pub end_date: Option<NaiveDate>,
    /// `*` = every strike.
    #[serde(default = "default_strike")]
    pub strike: String,
    /// `both` = calls and puts.
    #[serde(default = "default_right")]
    pub right: String,
    /// Post-decode transforms applied before write (rename / drop / scale).
    #[serde(default)]
    pub transforms: crate::Transforms,
    /// Every other endpoint parameter, by registry name.
    ///
    /// The six fields above are the ones a work unit is *keyed* on —
    /// they decide what the file is called and how coverage groups it.
    /// But endpoints declare far more than six: `max_dte`,
    /// `strike_range`, `start_time`/`end_time`, `venue`, the greeks
    /// inputs (`rate_type`, `annual_dividend`, `version`, …). Those are
    /// request options rather than identity, so they live here as
    /// registry-named strings and are applied in
    /// [`Self::to_endpoint_spec`] to whichever of them the endpoint
    /// actually declares. A key the endpoint does not declare is
    /// ignored rather than rejected, which is what lets one saved
    /// preset carry across related endpoints.
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
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

/// The widest window the range endpoints accept. Asking for more is
/// rejected server-side with "Too many days between start and end date;
/// max 365 days allowed", so a longer request has to be split before it
/// is queued rather than discovered as a failure afterwards.
pub const MAX_RANGE_DAYS: i64 = 365;

/// Split `[start, end]` into consecutive windows no wider than
/// [`MAX_RANGE_DAYS`].
///
/// A window already within the limit comes back unchanged, so the
/// common case stays one task. Longer ones are divided into the fewest
/// pieces that fit and then balanced across them: a three-year request
/// becomes three requests of a year each, and a four-year one becomes
/// four evenly-sized requests rather than three full years plus a
/// three-day remainder. Same number of calls either way, but the
/// progress the user watches advances evenly and a failure costs a
/// proportionate slice of the work.
pub fn chunk_window(start: NaiveDate, end: NaiveDate) -> Vec<(NaiveDate, NaiveDate)> {
    if end < start {
        return vec![];
    }
    // Inclusive of both ends: Jan 1 to Jan 1 is one day of data.
    let total_days = (end - start).num_days() + 1;
    let max_per_chunk = MAX_RANGE_DAYS + 1;
    // `div_ceil` on integers is still unstable on the pinned toolchain.
    let ceil_div = |a: i64, b: i64| (a + b - 1) / b;
    let chunks = ceil_div(total_days, max_per_chunk).max(1);
    let per_chunk = ceil_div(total_days, chunks);

    let mut out = Vec::with_capacity(chunks as usize);
    let mut from = start;
    while from <= end {
        let candidate = from + chrono::Duration::days(per_chunk - 1);
        let to = if candidate < end { candidate } else { end };
        out.push((from, to));
        match to.succ_opt() {
            Some(next) => from = next,
            None => break,
        }
    }
    out
}

impl DataSpec {
    pub fn ymd(&self) -> String {
        self.date.format("%Y%m%d").to_string()
    }

    /// The end of the window, as the wire spells dates. A spec with no
    /// explicit end covers a single day.
    pub fn end_ymd(&self) -> String {
        self.end_date
            .unwrap_or(self.date)
            .format("%Y%m%d")
            .to_string()
    }

    /// The part of a filename that distinguishes this pull from another
    /// of the same dataset, symbol and date.
    ///
    /// Empty for a whole-chain default, so those files keep the name
    /// they have always had and an existing library still scans. A
    /// narrowed pull earns a suffix: without one, requesting one
    /// expiration and then another writes to the same path, and the
    /// second task sees a file already there and reports done having
    /// fetched nothing.
    pub fn file_qualifier(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.expiration != "*" && !self.expiration.is_empty() {
            parts.push(format!("e{}", sanitize(&self.expiration)));
        }
        if self.strike != "*" && !self.strike.is_empty() {
            parts.push(format!("k{}", sanitize(&self.strike)));
        }
        let right = self.right.to_lowercase();
        if right != "both" && right != "*" && !right.is_empty() {
            parts.push(sanitize(&right));
        }
        // A window is part of what makes the pull distinct: the same
        // dataset over two different ranges is two different files.
        if let Some(end) = self.end_date {
            if end != self.date {
                parts.push(format!("to{}", end.format("%Y%m%d")));
            }
        }
        if let Some(interval) = self.interval.as_deref() {
            let normalized = normalize_interval(interval);
            if normalized != "tick" && !normalized.is_empty() {
                parts.push(sanitize(&normalized));
            }
        }
        // Same reasoning as the fields above: a pull narrowed by
        // `strike_range=5` is not the same data as the unrestricted
        // one, so it cannot share a filename with it.
        for (k, v) in &self.extra {
            if v.is_empty() || v == "*" {
                continue;
            }
            parts.push(format!("{}-{}", sanitize(k), sanitize(v)));
        }
        parts.join("_")
    }

    /// Stable file stem used everywhere on disk. The date stays last so
    /// `coverage::scan` can keep reading it off the end.
    pub fn file_stem(&self) -> String {
        let qualifier = self.file_qualifier();
        let middle = if qualifier.is_empty() {
            String::new()
        } else {
            format!("{qualifier}_")
        };
        format!(
            "{}_{}_{}{}",
            self.symbol.to_lowercase(),
            self.kind.as_str(),
            middle,
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
                // The range shape. Filling only `date` meant every
                // endpoint that declares a window and no single day —
                // the EOD datasets, the greeks EOD datasets, both
                // at-time datasets — reached the dispatcher without the
                // arguments it declares as required and failed, every
                // task, every time.
                "start_date" => Some(self.ymd()),
                "end_date" => Some(self.end_ymd()),
                other => self.extra.get(other).cloned(),
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

/// Strip anything that would be awkward in a filename on any platform.
fn sanitize(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_lowercase()
}

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

    /// Two pulls that differ only by contract filter must not write to
    /// the same file. Before this, the second one found the first one's
    /// file already there and reported done without fetching anything.
    fn option_spec() -> DataSpec {
        DataSpec {
            kind: DataKind::parse("option_history_quote").expect("known dataset"),
            symbol: "SPXW".into(),
            date: NaiveDate::from_ymd_opt(2026, 9, 21).unwrap(),
            interval: None,
            expiration: "*".into(),
            strike: "*".into(),
            right: "both".into(),
            end_date: None,
            transforms: crate::Transforms::default(),
            extra: BTreeMap::new(),
        }
    }

    /// `strike_range`, `max_dte`, `start_time` and the greeks inputs are
    /// real declared parameters that the UI collects. They used to stop
    /// at the queue: `to_endpoint_spec` filled six names and dropped
    /// everything else, so a user who asked for 5 strikes around spot
    /// silently downloaded the whole chain.
    /// The EOD datasets, the greeks EOD datasets and both at-time
    /// datasets declare `start_date`/`end_date` and no `date`. Filling
    /// only `date` meant every task for them reached the dispatcher
    /// without the arguments the registry marks required, and failed
    /// with "missing required arg 'start_date'" — 100% of the time,
    /// for every symbol and every window.
    #[test]
    fn a_window_inside_the_limit_stays_one_task() {
        let a = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let b = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();
        assert_eq!(chunk_window(a, b), vec![(a, b)]);
    }

    /// Three years of daily history is four calls, not one rejected
    /// one and not 751 malformed ones.
    #[test]
    fn a_longer_window_is_split_at_the_server_limit() {
        let a = NaiveDate::from_ymd_opt(2023, 9, 22).unwrap();
        let b = NaiveDate::from_ymd_opt(2026, 9, 22).unwrap();
        let chunks = chunk_window(a, b);

        assert_eq!(
            chunks.len(),
            3,
            "three years is three requests, not one rejected one"
        );
        assert_eq!(chunks.first().unwrap().0, a, "starts where asked");
        assert_eq!(chunks.last().unwrap().1, b, "ends where asked");

        // Balanced: no window is more than a day off any other, so the
        // split does not leave a one-day tail.
        let widths: Vec<i64> = chunks
            .iter()
            .map(|(f, t)| (*t - *f).num_days() + 1)
            .collect();
        let (lo, hi) = (widths.iter().min().unwrap(), widths.iter().max().unwrap());
        assert!(hi - lo <= 1, "windows are lopsided: {widths:?}");
        assert_eq!(widths.iter().sum::<i64>(), (b - a).num_days() + 1);
        for (from, to) in &chunks {
            assert!(
                (*to - *from).num_days() <= MAX_RANGE_DAYS,
                "{from}..{to} is wider than the server accepts"
            );
        }
        // Contiguous and non-overlapping: no day is fetched twice and
        // none is skipped.
        for pair in chunks.windows(2) {
            assert_eq!(pair[0].1.succ_opt().unwrap(), pair[1].0);
        }
    }

    #[test]
    fn a_backwards_window_yields_nothing() {
        let a = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();
        let b = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        assert!(chunk_window(a, b).is_empty());
    }

    #[test]
    fn range_endpoints_receive_the_window_they_declare() {
        let mut spec = option_spec();
        spec.kind = DataKind::parse("stock_history_eod").expect("known dataset");
        spec.date = NaiveDate::from_ymd_opt(2023, 9, 22).unwrap();
        spec.end_date = NaiveDate::from_ymd_opt(2026, 9, 22);

        let lowered = spec.to_endpoint_spec();
        assert_eq!(
            lowered.args.get("start_date").map(String::as_str),
            Some("20230922")
        );
        assert_eq!(
            lowered.args.get("end_date").map(String::as_str),
            Some("20260922")
        );

        // Every argument the endpoint marks required must be present,
        // whatever the shape.
        let meta = thetadatadx::find("stock_history_eod").expect("registry knows it");
        for p in meta.params.iter().filter(|p| p.required) {
            assert!(
                lowered.args.contains_key(p.name),
                "required arg `{}` never reaches the request",
                p.name
            );
        }
    }

    /// A spec with no explicit end covers one day, so a single-day pull
    /// through a range endpoint still asks for a valid window.
    #[test]
    fn a_dateless_spec_asks_for_a_single_day_window() {
        let mut spec = option_spec();
        spec.kind = DataKind::parse("stock_history_eod").expect("known dataset");
        let lowered = spec.to_endpoint_spec();
        assert_eq!(lowered.args.get("start_date"), lowered.args.get("end_date"));
    }

    /// Two windows over the same dataset are two different files.
    #[test]
    fn the_window_is_part_of_the_filename() {
        let mut a = option_spec();
        a.kind = DataKind::parse("stock_history_eod").expect("known dataset");
        let mut b = a.clone();
        a.end_date = NaiveDate::from_ymd_opt(2026, 1, 1);
        b.end_date = NaiveDate::from_ymd_opt(2026, 6, 1);
        assert_ne!(a.file_stem(), b.file_stem());
    }

    #[test]
    fn extra_args_reach_the_endpoint_when_it_declares_them() {
        let mut spec = option_spec();
        spec.extra.insert("strike_range".into(), "5".into());
        spec.extra.insert("max_dte".into(), "30".into());

        let lowered = spec.to_endpoint_spec();
        assert_eq!(
            lowered.args.get("strike_range").map(String::as_str),
            Some("5")
        );
        assert_eq!(lowered.args.get("max_dte").map(String::as_str), Some("30"));
    }

    /// An endpoint that does not declare the parameter must not receive
    /// it — `insert_raw` would reject the unknown name and fail the
    /// whole task. This is what lets one saved preset carry across
    /// related endpoints.
    #[test]
    fn extra_args_the_endpoint_does_not_declare_are_dropped() {
        let mut spec = option_spec();
        spec.kind = DataKind::parse("stock_history_trade").expect("known dataset");
        spec.extra.insert("strike_range".into(), "5".into());

        let lowered = spec.to_endpoint_spec();
        assert!(
            !lowered.args.contains_key("strike_range"),
            "stock_history_trade has no strike_range param"
        );
    }

    /// Narrowing by an extra arg produces different data, so it must
    /// produce a different file — same reasoning as the expiration and
    /// strike qualifiers.
    #[test]
    fn extra_args_change_the_filename() {
        let plain = option_spec();
        let mut narrowed = option_spec();
        narrowed.extra.insert("strike_range".into(), "5".into());

        assert_ne!(plain.file_stem(), narrowed.file_stem());
        assert!(
            narrowed.file_stem().ends_with("_20260921"),
            "date stays last"
        );
    }

    #[test]
    fn narrowed_pulls_get_distinct_filenames() {
        let base = DataSpec {
            kind: DataKind::parse("option_history_quote").expect("known dataset"),
            symbol: "SPXW".into(),
            date: NaiveDate::from_ymd_opt(2026, 9, 21).unwrap(),
            interval: None,
            expiration: "*".into(),
            strike: "*".into(),
            right: "both".into(),
            end_date: None,
            transforms: crate::Transforms::default(),
            extra: BTreeMap::new(),
        };

        // The whole-chain default keeps the historical name, so an
        // existing library still resolves.
        assert_eq!(base.file_qualifier(), "");
        assert_eq!(base.file_stem(), "spxw_option_history_quote_20260921");

        let mut a = base.clone();
        a.expiration = "20261016".into();
        let mut b = base.clone();
        b.expiration = "20261120".into();
        assert_ne!(a.file_stem(), b.file_stem());

        let mut calls = base.clone();
        calls.right = "call".into();
        let mut puts = base.clone();
        puts.right = "put".into();
        assert_ne!(calls.file_stem(), puts.file_stem());

        let mut tick = base.clone();
        tick.interval = Some("tick".into());
        let mut minute = base.clone();
        minute.interval = Some("1m".into());
        // `tick` is the unsampled default and adds nothing to the name.
        assert_eq!(tick.file_stem(), base.file_stem());
        assert_ne!(minute.file_stem(), base.file_stem());
    }

    /// The date has to stay the last underscore-separated segment,
    /// because `coverage::scan` reads it off the end.
    #[test]
    fn the_date_stays_last_whatever_the_qualifier() {
        let mut spec = DataSpec {
            kind: DataKind::parse("option_history_quote").expect("known dataset"),
            symbol: "SPXW".into(),
            date: NaiveDate::from_ymd_opt(2026, 9, 21).unwrap(),
            interval: Some("5m".into()),
            expiration: "20261016".into(),
            strike: "5400.5".into(),
            right: "call".into(),
            end_date: None,
            transforms: crate::Transforms::default(),
            extra: BTreeMap::new(),
        };
        let stem = spec.file_stem();
        assert!(stem.ends_with("_20260921"), "date must be last: {stem}");
        assert!(stem.starts_with("spxw_"), "symbol must be first: {stem}");
        // Nothing in a filename that a filesystem would object to.
        assert!(
            stem.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "unsafe filename: {stem}"
        );

        spec.strike = "*".into();
        assert!(!spec.file_stem().contains("k-"));
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

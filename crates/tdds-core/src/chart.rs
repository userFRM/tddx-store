//! Summarise one downloaded file into a chart's worth of points.
//!
//! The question a chart answers here is narrow on purpose: *did I get
//! what I asked for?* A trade file with a two-hour hole in it looks
//! identical to a complete one in a row table and obvious in a chart,
//! so alongside the series this reports the gaps it found.
//!
//! The chart follows the columns, never a setting the user has to
//! reason about: `open/high/low/close` is drawn as candles; a `price`
//! as a line with its high–low envelope; `bid`/`ask` as a band beneath
//! either. Everything is bucketed down to screen resolution — a day of
//! one-second bars is 23,000 rows, and a day of trades far more.

use std::fs::File;
use std::path::Path;

use arrow_array::{Array, ArrayRef, RecordBatch};
use arrow_schema::DataType;
use chrono::Datelike;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde::Serialize;

/// How many candles to aim for when the caller does not say: enough to
/// read, few enough that each is wide enough to see.
pub const DEFAULT_TARGET: usize = 120;

const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    /// `open/high/low/close` present.
    Candles,
    /// A single price, drawn as a line within its high–low envelope.
    Line,
    /// Only a bid and ask: the quoted market, with no trades.
    Band,
}

/// A stretch of time where the file has no rows although the data
/// around it says it should.
#[derive(Debug, Clone, Serialize)]
pub struct Gap {
    pub from_ms: i64,
    pub to_ms: i64,
}

/// How long one candle spans. Aligned to the calendar, the way every
/// trading chart does it: a 5-minute candle starts on :00, :05, :10; a
/// weekly one on Monday; a monthly one on the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Ms(i64),
    Week,
    Month,
}

impl Step {
    /// Roughly how long, for choosing between steps. Calendar months
    /// vary; thirty days is close enough to count candles.
    fn approx_ms(self) -> i64 {
        match self {
            Step::Ms(ms) => ms,
            Step::Week => 7 * DAY_MS,
            Step::Month => 30 * DAY_MS,
        }
    }

    /// The start of the candle `t` falls in.
    fn floor(self, t: i64) -> i64 {
        match self {
            Step::Ms(ms) => t.div_euclid(ms) * ms,
            Step::Week | Step::Month => {
                let day = t.div_euclid(DAY_MS);
                let Some(date) =
                    chrono::DateTime::from_timestamp_millis(day * DAY_MS).map(|d| d.date_naive())
                else {
                    return day * DAY_MS;
                };
                let start = match self {
                    Step::Week => {
                        date - chrono::Duration::days(i64::from(
                            date.weekday().num_days_from_monday(),
                        ))
                    }
                    _ => date.with_day(1).unwrap_or(date),
                };
                start
                    .and_hms_opt(0, 0, 0)
                    .map_or(day * DAY_MS, |d| d.and_utc().timestamp_millis())
            }
        }
    }
}

/// The candle intervals the chart offers, finest first.
const STEPS: &[(&str, Step)] = &[
    ("1s", Step::Ms(1_000)),
    ("5s", Step::Ms(5_000)),
    ("15s", Step::Ms(15_000)),
    ("30s", Step::Ms(30_000)),
    ("1m", Step::Ms(60_000)),
    ("5m", Step::Ms(300_000)),
    ("15m", Step::Ms(900_000)),
    ("30m", Step::Ms(1_800_000)),
    ("1h", Step::Ms(3_600_000)),
    ("1D", Step::Ms(DAY_MS)),
    ("1W", Step::Week),
    ("1M", Step::Month),
];

/// Fewer candles than this and the chart says nothing.
const MIN_CANDLES: i64 = 8;
/// More than this and no width can show them as candles.
const MAX_CANDLES: i64 = 1_500;

#[derive(Debug, Serialize)]
pub struct ChartSeries {
    pub shape: Shape,
    /// Rows in the file, before bucketing.
    pub rows: usize,
    /// Whether the x axis is whole days (EOD) or intraday time.
    pub daily: bool,
    /// Bucket start, milliseconds since the Unix epoch, exchange-local
    /// wall time (the files carry no zone; this keeps 09:30 at 09:30).
    pub x: Vec<i64>,
    pub open: Vec<Option<f64>>,
    pub high: Vec<Option<f64>>,
    pub low: Vec<Option<f64>>,
    pub close: Vec<Option<f64>>,
    pub bid: Vec<Option<f64>>,
    pub ask: Vec<Option<f64>>,
    pub volume: Vec<Option<f64>>,
    pub gaps: Vec<Gap>,
    /// The interval each candle spans, e.g. `5m`, `1D`.
    pub step: String,
    /// Every interval this file can sensibly be drawn at: no finer than
    /// the data itself, and yielding between a handful of candles and
    /// what a screen can hold.
    pub steps: Vec<String>,
}

/// One row, reduced to what a chart draws.
struct Point {
    t: i64,
    open: Option<f64>,
    high: Option<f64>,
    low: Option<f64>,
    close: Option<f64>,
    bid: Option<f64>,
    ask: Option<f64>,
    volume: Option<f64>,
}

/// Summarise `path` into candles of `step` (e.g. `"5m"`), or, when
/// `step` is `None` or not offered for this file, the finest interval
/// that fits in `target` candles — so the chart breathes at whatever
/// width it is drawn.
pub fn series(path: &Path, step: Option<&str>, target: usize) -> crate::Result<ChartSeries> {
    if path.extension().and_then(|e| e.to_str()) != Some("parquet") {
        return Err(crate::Error::Other(
            "the chart reads Parquet files; this one was written in another format".into(),
        ));
    }
    let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?;
    let reader = builder.with_batch_size(8_192).build()?;

    let mut points: Vec<Point> = Vec::new();
    let mut shape: Option<Shape> = None;
    let mut daily = true;
    for batch in reader {
        let batch = batch?;
        let cols = Columns::find(&batch);
        let s = cols.shape().ok_or_else(|| {
            crate::Error::Other(
                "nothing to chart: the file has no price, OHLC or bid/ask columns".into(),
            )
        })?;
        shape.get_or_insert(s);
        if cols.ms_of_day.is_some() {
            daily = false;
        }
        for i in 0..batch.num_rows() {
            let Some(t) = cols.time_ms(i) else { continue };
            // A bar with no trades in it is written as all zeros. Zero
            // is "nothing traded", not a price: kept, it drags the
            // axis to 0 and flattens the chart, and it hides real
            // holes from the gap check, since the row is still there.
            let px = |c: &Option<ArrayRef>| c.as_ref().and_then(|c| num(c, i)).filter(|v| *v > 0.0);
            let price = px(&cols.price);
            let point = Point {
                t,
                open: px(&cols.open).or(price),
                high: px(&cols.high).or(price),
                low: px(&cols.low).or(price),
                close: px(&cols.close).or(price),
                bid: px(&cols.bid),
                ask: px(&cols.ask),
                volume: cols.volume.as_ref().and_then(|c| num(c, i)),
            };
            if point.close.is_none() && point.bid.is_none() && point.ask.is_none() {
                continue;
            }
            points.push(point);
        }
    }
    let shape = shape.unwrap_or(Shape::Line);
    points.sort_by_key(|p| p.t);
    let rows = points.len();
    let gaps = find_gaps(&points, daily);
    let (step_name, steps, buckets) = choose_and_bucket(&points, daily, step, target.max(2));

    let mut out = ChartSeries {
        shape,
        rows,
        daily,
        x: Vec::with_capacity(buckets.len()),
        open: Vec::with_capacity(buckets.len()),
        high: Vec::with_capacity(buckets.len()),
        low: Vec::with_capacity(buckets.len()),
        close: Vec::with_capacity(buckets.len()),
        bid: Vec::with_capacity(buckets.len()),
        ask: Vec::with_capacity(buckets.len()),
        volume: Vec::with_capacity(buckets.len()),
        gaps,
        step: step_name,
        steps,
    };
    for b in buckets {
        out.x.push(b.t);
        out.open.push(b.open);
        out.high.push(b.high);
        out.low.push(b.low);
        out.close.push(b.close);
        out.bid.push(b.bid);
        out.ask.push(b.ask);
        out.volume.push(b.volume);
    }
    Ok(out)
}

/// Which intervals suit these points, which one to use, and the
/// candles at that interval.
fn choose_and_bucket(
    points: &[Point],
    daily: bool,
    wanted: Option<&str>,
    target: usize,
) -> (String, Vec<String>, Vec<Point>) {
    if points.len() < 2 {
        return (
            "—".into(),
            Vec::new(),
            points.iter().map(|p| Point { ..*p }).collect(),
        );
    }
    let native = median_spacing(points);

    // Counted, not estimated from the time span: weekends and nights
    // have no candles, so a span-based estimate put a year of days at
    // 365 when it is 260, and rejected daily candles that would fit.
    let offered: Vec<(&str, Step, i64)> = STEPS
        .iter()
        .filter_map(|&(name, st)| {
            let ms = st.approx_ms();
            let finer_than_data = ms * 10 < native * 9;
            let sub_day_on_daily = daily && ms < DAY_MS;
            if finer_than_data || sub_day_on_daily {
                return None;
            }
            let n = count_candles(points, st);
            (MIN_CANDLES..=MAX_CANDLES)
                .contains(&n)
                .then_some((name, st, n))
        })
        .collect();

    // What was asked for, if this file offers it; otherwise the finest
    // interval that fits the width; otherwise the coarsest there is.
    let chosen = wanted
        .and_then(|w| offered.iter().find(|(n, ..)| *n == w))
        .or_else(|| offered.iter().find(|(.., n)| *n <= target as i64))
        .or_else(|| offered.last())
        .map(|&(name, st, _)| (name, st));

    let names = offered.iter().map(|(n, ..)| (*n).to_string()).collect();
    match chosen {
        Some((name, st)) => (name.to_string(), names, bucket_by(points, st)),
        // Too little data to offer any interval: draw it as it is.
        None => (
            "raw".into(),
            names,
            points.iter().map(|p| Point { ..*p }).collect(),
        ),
    }
}

/// How many candles `points` make at `step`: one per distinct candle
/// start, since the points are in time order.
fn count_candles(points: &[Point], step: Step) -> i64 {
    let mut n = 0;
    let mut last = None;
    for p in points {
        let start = step.floor(p.t);
        if last != Some(start) {
            n += 1;
            last = Some(start);
        }
    }
    n
}

/// Median time between consecutive rows.
fn median_spacing(points: &[Point]) -> i64 {
    let mut d: Vec<i64> = points
        .windows(2)
        .map(|w| w[1].t - w[0].t)
        .filter(|d| *d > 0)
        .collect();
    if d.is_empty() {
        return 1;
    }
    d.sort_unstable();
    d[d.len() / 2]
}

/// Merge points into calendar-aligned candles of `step`. Open is the
/// first, close the last, high and low the extremes, volume the sum;
/// bid and ask are the last quote in the candle. A candle with no rows
/// is not emitted, so a hole in the data stays a hole.
fn bucket_by(points: &[Point], step: Step) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::new();
    for p in points {
        let start = step.floor(p.t);
        match out.last_mut() {
            Some(b) if b.t == start => {
                b.high = max_opt(b.high, p.high);
                b.low = min_opt(b.low, p.low);
                b.close = p.close.or(b.close);
                b.bid = p.bid.or(b.bid);
                b.ask = p.ask.or(b.ask);
                b.volume = sum_opt(b.volume, p.volume);
            }
            _ => out.push(Point { t: start, ..*p }),
        }
    }
    out
}

/// Stretches with no rows where the file's own rhythm says there should
/// be some.
///
/// The rhythm is the median spacing between rows. For intraday data a
/// gap is ten times that, and at least a minute — a quiet stock can go
/// seconds between trades without anything being missing. Overnight is
/// never a gap. For daily data a gap is more than four calendar days,
/// so a weekend, and a holiday beside one, are not flagged.
fn find_gaps(points: &[Point], daily: bool) -> Vec<Gap> {
    if points.len() < 3 {
        return Vec::new();
    }
    let mut deltas: Vec<i64> = points
        .windows(2)
        .map(|w| w[1].t - w[0].t)
        .filter(|d| *d > 0)
        .collect();
    if deltas.is_empty() {
        return Vec::new();
    }
    deltas.sort_unstable();
    let median = deltas[deltas.len() / 2];
    let threshold = if daily {
        4 * DAY_MS + 1
    } else {
        (median * 10).max(60_000)
    };

    let mut gaps: Vec<Gap> = points
        .windows(2)
        .filter(|w| {
            let d = w[1].t - w[0].t;
            let overnight = !daily && w[0].t.div_euclid(DAY_MS) != w[1].t.div_euclid(DAY_MS);
            d > threshold && !overnight
        })
        .map(|w| Gap {
            from_ms: w[0].t,
            to_ms: w[1].t,
        })
        .collect();
    // The widest first, and only as many as a person will read.
    gaps.sort_by_key(|g| std::cmp::Reverse(g.to_ms - g.from_ms));
    gaps.truncate(12);
    gaps.sort_by_key(|g| g.from_ms);
    gaps
}

fn max_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    }
}
fn min_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, None) => x,
        (None, y) => y,
    }
}
fn sum_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (None, None) => None,
        (x, y) => Some(x.unwrap_or(0.0) + y.unwrap_or(0.0)),
    }
}

/// The columns a chart cares about, located by name in one batch.
struct Columns {
    date: Option<ArrayRef>,
    ms_of_day: Option<ArrayRef>,
    open: Option<ArrayRef>,
    high: Option<ArrayRef>,
    low: Option<ArrayRef>,
    close: Option<ArrayRef>,
    price: Option<ArrayRef>,
    bid: Option<ArrayRef>,
    ask: Option<ArrayRef>,
    volume: Option<ArrayRef>,
}

impl Columns {
    fn find(batch: &RecordBatch) -> Self {
        let get = |name: &str| batch.column_by_name(name).cloned();
        Self {
            date: get("date"),
            ms_of_day: get("ms_of_day"),
            open: get("open"),
            high: get("high"),
            low: get("low"),
            close: get("close"),
            price: get("price"),
            bid: get("bid"),
            ask: get("ask"),
            volume: get("volume").or_else(|| get("size")),
        }
    }

    fn shape(&self) -> Option<Shape> {
        if self.open.is_some() && self.high.is_some() && self.low.is_some() && self.close.is_some()
        {
            Some(Shape::Candles)
        } else if self.price.is_some() {
            Some(Shape::Line)
        } else if self.bid.is_some() && self.ask.is_some() {
            Some(Shape::Band)
        } else {
            None
        }
    }

    /// `date` (YYYYMMDD) plus `ms_of_day` when present.
    fn time_ms(&self, i: usize) -> Option<i64> {
        let ymd = self.date.as_ref().and_then(|c| num(c, i))? as i64;
        let date = chrono::NaiveDate::from_ymd_opt(
            (ymd / 10_000) as i32,
            ((ymd / 100) % 100) as u32,
            (ymd % 100) as u32,
        )?;
        let base = date.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis();
        let ms = self
            .ms_of_day
            .as_ref()
            .and_then(|c| num(c, i))
            .unwrap_or(0.0) as i64;
        Some(base + ms)
    }
}

/// A numeric cell as `f64`, whatever integer or float width it was
/// written with.
fn num(col: &ArrayRef, i: usize) -> Option<f64> {
    use arrow_array::cast::AsArray;
    use arrow_array::types::*;
    if col.is_null(i) {
        return None;
    }
    Some(match col.data_type() {
        DataType::Float64 => col.as_primitive::<Float64Type>().value(i),
        DataType::Float32 => col.as_primitive::<Float32Type>().value(i) as f64,
        DataType::Int64 => col.as_primitive::<Int64Type>().value(i) as f64,
        DataType::Int32 => col.as_primitive::<Int32Type>().value(i) as f64,
        DataType::Int16 => col.as_primitive::<Int16Type>().value(i) as f64,
        DataType::UInt64 => col.as_primitive::<UInt64Type>().value(i) as f64,
        DataType::UInt32 => col.as_primitive::<UInt32Type>().value(i) as f64,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(t: i64, price: f64) -> Point {
        Point {
            t,
            open: Some(price),
            high: Some(price),
            low: Some(price),
            close: Some(price),
            bid: None,
            ask: None,
            volume: Some(1.0),
        }
    }

    const NINE_THIRTY: i64 = 34_200_000;

    #[test]
    fn a_hole_in_an_intraday_file_is_reported() {
        // A bar every second, then nothing for two hours, then resumes.
        let mut pts: Vec<Point> = (0..600)
            .map(|s| p(NINE_THIRTY + s * 1_000, 100.0))
            .collect();
        let resume = NINE_THIRTY + 599_000 + 2 * 3_600_000;
        pts.extend((0..600).map(|s| p(resume + s * 1_000, 101.0)));

        let gaps = find_gaps(&pts, false);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].to_ms - gaps[0].from_ms, 2 * 3_600_000);
    }

    #[test]
    fn a_complete_intraday_file_has_no_gaps() {
        let pts: Vec<Point> = (0..2_000)
            .map(|s| p(NINE_THIRTY + s * 1_000, 100.0))
            .collect();
        assert!(find_gaps(&pts, false).is_empty());
    }

    /// Weekends and a holiday beside one are the market being shut, not
    /// the file being incomplete.
    #[test]
    fn weekends_are_not_gaps_in_daily_data() {
        let day = |d: i64| p(d * DAY_MS, 100.0);
        // Thu, Fri, (weekend), Mon, Tue, then a four-day weekend.
        let pts = vec![day(0), day(1), day(4), day(5), day(9)];
        assert!(find_gaps(&pts, true).is_empty());
        // A missing week is.
        let pts = vec![day(0), day(1), day(15), day(16)];
        assert_eq!(find_gaps(&pts, true).len(), 1);
    }

    /// The shape of `px` in `series`, exercised directly.
    #[test]
    fn a_zero_price_is_no_trade_not_a_price() {
        let px = |v: f64| Some(v).filter(|v| *v > 0.0);
        assert_eq!(px(0.0), None);
        assert_eq!(px(109.43), Some(109.43));
    }

    fn day(d: i64) -> Point {
        p(d * DAY_MS, 100.0 + d as f64)
    }

    /// A year of daily bars at a normal width: candles, not a smear.
    #[test]
    fn a_year_of_days_becomes_weekly_candles_when_days_would_not_fit() {
        let pts: Vec<Point> = (0..365).filter(|d| d % 7 < 5).map(day).collect();
        let (step, offered, candles) = choose_and_bucket(&pts, true, None, 120);
        assert_eq!(step, "1W", "260 days do not fit in 120; weeks do");
        assert!(candles.len() <= 60);
        assert!(
            offered.contains(&"1D".to_string()),
            "days are still on offer"
        );
        assert!(
            !offered
                .iter()
                .any(|s| s.ends_with('s') || s.ends_with('m') || s == "1h"),
            "no intraday interval for daily data: {offered:?}"
        );
    }

    #[test]
    fn a_wide_chart_keeps_daily_candles() {
        let pts: Vec<Point> = (0..365).filter(|d| d % 7 < 5).map(day).collect();
        let (step, _, candles) = choose_and_bucket(&pts, true, None, 300);
        assert_eq!(step, "1D");
        assert_eq!(candles.len(), pts.len());
    }

    #[test]
    fn an_explicit_interval_is_honoured_when_offered() {
        let pts: Vec<Point> = (0..365).filter(|d| d % 7 < 5).map(day).collect();
        let (step, _, candles) = choose_and_bucket(&pts, true, Some("1M"), 120);
        assert_eq!(step, "1M");
        assert!((12..=13).contains(&candles.len()));
        // Not offered — finer than the data — so the choice falls back.
        let (step, ..) = choose_and_bucket(&pts, true, Some("5m"), 120);
        assert_ne!(step, "5m");
    }

    /// A session of one-second bars offers minutes and up, never an
    /// interval finer than the data.
    #[test]
    fn intraday_intervals_start_at_the_data_resolution() {
        let pts: Vec<Point> = (0..23_400)
            .map(|s| p(NINE_THIRTY + s * 1_000, 100.0))
            .collect();
        let (step, offered, candles) = choose_and_bucket(&pts, false, None, 120);
        assert_eq!(step, "5m", "78 five-minute candles fit in 120");
        assert_eq!(candles.len(), 78);
        assert!(
            !offered.contains(&"1s".to_string()),
            "23,400 one-second candles cannot be drawn"
        );
    }

    #[test]
    fn weekly_candles_start_on_monday() {
        // 2024-01-03 is a Wednesday; its week starts Monday 2024-01-01.
        let wed = chrono::NaiveDate::from_ymd_opt(2024, 1, 3)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        let mon = chrono::NaiveDate::from_ymd_opt(2024, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        assert_eq!(Step::Week.floor(wed), mon);
    }

    #[test]
    fn bucketing_keeps_the_extremes() {
        let mut pts: Vec<Point> = (0..10_000).map(|s| p(s * 1_000, 100.0)).collect();
        pts[4_321].high = Some(250.0);
        pts[4_321].low = Some(250.0);
        pts[7_777].low = Some(1.0);
        pts[7_777].high = Some(1.0);

        // 10,000 one-second rows at 5-minute candles.
        let b = bucket_by(&pts, Step::Ms(300_000));
        assert!(b.len() <= 35);
        let hi = b.iter().filter_map(|x| x.high).fold(f64::MIN, f64::max);
        let lo = b.iter().filter_map(|x| x.low).fold(f64::MAX, f64::min);
        assert_eq!(hi, 250.0, "a spike must survive downsampling");
        assert_eq!(lo, 1.0, "so must a dip");
        let vol: f64 = b.iter().filter_map(|x| x.volume).sum();
        assert_eq!(vol, 10_000.0, "volume is summed, not sampled");
    }
}

//! Recurring scheduled downloads, stored alongside the queue.
//!
//! A row says "on these days, at HH:MM, queue this dataset". `at_time`
//! is read in the machine's own timezone — the field is entered and
//! displayed without one, and a desktop app that fired at a different
//! hour than the clock on the wall would be surprising. Data
//! availability is an Eastern-time question, so leave headroom after
//! the close rather than scheduling on the boundary.
//!
//! Firing is driven by the desktop app's ticker (`schedule::due`), not
//! from inside this module.

use chrono::{DateTime, Datelike, Local, NaiveTime, Utc, Weekday};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::format::OutputFormat;
use crate::spec::DataKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub name: String,
    pub kind: DataKind,
    pub symbol: String,
    pub format: OutputFormat,
    /// Which days the schedule runs on. See [`Cadence`].
    pub cron_kind: String,
    pub at_time: String, // "HH:MM", in the machine's timezone
    pub last_fired_at: Option<i64>,
    pub paused: bool,
    pub created_at: i64,
}

/// Which days a schedule runs on, parsed from `Schedule::cron_kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cadence {
    /// Every day.
    Daily,
    /// Monday through Friday.
    Weekdays,
    /// One named weekday, e.g. `weekly:mon`.
    Weekly(Weekday),
}

impl Cadence {
    /// Parse `daily` | `weekdays` | `weekly:<mon..sun>`. An unrecognised
    /// value falls back to `Daily`: over-running is visible in the queue
    /// and fixable, while silently never running is not.
    pub fn parse(raw: &str) -> Self {
        let raw = raw.trim().to_ascii_lowercase();
        if let Some(day) = raw.strip_prefix("weekly:") {
            return match day.trim() {
                "mon" | "monday" => Cadence::Weekly(Weekday::Mon),
                "tue" | "tuesday" => Cadence::Weekly(Weekday::Tue),
                "wed" | "wednesday" => Cadence::Weekly(Weekday::Wed),
                "thu" | "thursday" => Cadence::Weekly(Weekday::Thu),
                "fri" | "friday" => Cadence::Weekly(Weekday::Fri),
                "sat" | "saturday" => Cadence::Weekly(Weekday::Sat),
                "sun" | "sunday" => Cadence::Weekly(Weekday::Sun),
                _ => Cadence::Daily,
            };
        }
        match raw.as_str() {
            "weekdays" => Cadence::Weekdays,
            _ => Cadence::Daily,
        }
    }

    /// Does this cadence include `day`?
    pub fn covers(self, day: Weekday) -> bool {
        match self {
            Cadence::Daily => true,
            Cadence::Weekdays => !matches!(day, Weekday::Sat | Weekday::Sun),
            Cadence::Weekly(w) => day == w,
        }
    }
}

impl Schedule {
    pub fn at(&self) -> Option<NaiveTime> {
        NaiveTime::parse_from_str(&self.at_time, "%H:%M").ok()
    }

    pub fn cadence(&self) -> Cadence {
        Cadence::parse(&self.cron_kind)
    }

    /// Should this schedule fire at `now`? True when today is a day the
    /// cadence covers, today's fire-time has passed, and the row has not
    /// already fired within the refire window.
    ///
    /// `now` is converted to the machine's timezone before the date and
    /// time-of-day comparisons; comparing against UTC fired a 09:30
    /// schedule five hours early for a user on the US east coast.
    pub fn should_fire(&self, now: DateTime<Utc>) -> bool {
        if self.paused {
            return false;
        }
        let Some(time) = self.at() else {
            return false;
        };
        let local = now.with_timezone(&Local).naive_local();
        if !self.cadence().covers(local.date().weekday()) {
            return false;
        }
        if local.time() < time {
            return false;
        }
        match self.last_fired_at {
            None => true,
            Some(prev) => now.timestamp() - prev > crate::config::SCHEDULE_MIN_REFIRE_SECS,
        }
    }
}

pub async fn create_table(pool: &SqlitePool) -> crate::Result<()> {
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS schedules (
            id            TEXT PRIMARY KEY,
            name          TEXT NOT NULL,
            kind          TEXT NOT NULL,
            symbol        TEXT NOT NULL,
            format        TEXT NOT NULL,
            cron_kind     TEXT NOT NULL,
            at_time       TEXT NOT NULL,
            last_fired_at INTEGER,
            paused        INTEGER NOT NULL DEFAULT 0,
            created_at    INTEGER NOT NULL
        )"#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert(pool: &SqlitePool, s: &Schedule) -> crate::Result<()> {
    sqlx::query(
        r#"INSERT INTO schedules (id, name, kind, symbol, format, cron_kind, at_time,
                                  last_fired_at, paused, created_at)
           VALUES (?,?,?,?,?,?,?,?,?,?)"#,
    )
    .bind(&s.id)
    .bind(&s.name)
    .bind(s.kind.as_str())
    .bind(&s.symbol)
    .bind(s.format.extension())
    .bind(&s.cron_kind)
    .bind(&s.at_time)
    .bind(s.last_fired_at)
    .bind(s.paused as i64)
    .bind(s.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list(pool: &SqlitePool) -> crate::Result<Vec<Schedule>> {
    let rows: Vec<ScheduleRow> = sqlx::query_as("SELECT * FROM schedules ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    rows.into_iter().map(TryInto::try_into).collect()
}

/// Every schedule that is due to fire at `now`. The caller enqueues the
/// work and calls [`mark_fired`]; this function has no side effects, so
/// a caller that fails partway does not lose the rest of the batch.
pub async fn due(pool: &SqlitePool, now: DateTime<Utc>) -> crate::Result<Vec<Schedule>> {
    Ok(list(pool)
        .await?
        .into_iter()
        .filter(|s| s.should_fire(now))
        .collect())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> crate::Result<()> {
    sqlx::query("DELETE FROM schedules WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_paused(pool: &SqlitePool, id: &str, paused: bool) -> crate::Result<()> {
    sqlx::query("UPDATE schedules SET paused = ? WHERE id = ?")
        .bind(paused as i64)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_fired(pool: &SqlitePool, id: &str, when: i64) -> crate::Result<()> {
    sqlx::query("UPDATE schedules SET last_fired_at = ? WHERE id = ?")
        .bind(when)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct ScheduleRow {
    id: String,
    name: String,
    kind: String,
    symbol: String,
    format: String,
    cron_kind: String,
    at_time: String,
    last_fired_at: Option<i64>,
    paused: i64,
    created_at: i64,
}

impl TryFrom<ScheduleRow> for Schedule {
    type Error = crate::Error;
    fn try_from(r: ScheduleRow) -> std::result::Result<Self, Self::Error> {
        let kind = DataKind::parse(&r.kind)
            .ok_or_else(|| crate::Error::Other(format!("bad kind {}", r.kind)))?;
        let format = OutputFormat::parse(&r.format)
            .ok_or_else(|| crate::Error::Other(format!("bad format {}", r.format)))?;
        Ok(Schedule {
            id: r.id,
            name: r.name,
            kind,
            symbol: r.symbol,
            format,
            cron_kind: r.cron_kind,
            at_time: r.at_time,
            last_fired_at: r.last_fired_at,
            paused: r.paused != 0,
            created_at: r.created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn schedule(cron_kind: &str, at_time: &str) -> Schedule {
        Schedule {
            id: "s1".into(),
            name: "test".into(),
            kind: DataKind::parse("stock_history_eod").expect("registry knows the dataset"),
            symbol: "QQQ".into(),
            format: OutputFormat::Parquet,
            cron_kind: cron_kind.into(),
            at_time: at_time.into(),
            last_fired_at: None,
            paused: false,
            created_at: 0,
        }
    }

    /// `now` at the given local wall-clock time, whatever timezone the
    /// test host runs in. The comparisons under test are local ones, so
    /// building the instant from a local time is what exercises them.
    fn local(y: i32, m: u32, d: u32, hh: u32, mm: u32) -> DateTime<Utc> {
        Local
            .with_ymd_and_hms(y, m, d, hh, mm, 0)
            .single()
            .expect("unambiguous local time")
            .with_timezone(&Utc)
    }

    #[test]
    fn cadence_parses_every_documented_spelling() {
        assert_eq!(Cadence::parse("daily"), Cadence::Daily);
        assert_eq!(Cadence::parse("weekdays"), Cadence::Weekdays);
        assert_eq!(Cadence::parse("weekly:mon"), Cadence::Weekly(Weekday::Mon));
        assert_eq!(Cadence::parse("WEEKLY:FRI"), Cadence::Weekly(Weekday::Fri));
        // An unknown value runs daily rather than never.
        assert_eq!(Cadence::parse("hourly"), Cadence::Daily);
    }

    #[test]
    fn weekdays_does_not_fire_at_the_weekend() {
        let s = schedule("weekdays", "17:30");
        // 2026-09-18 is a Friday, 2026-09-19 a Saturday.
        assert!(s.should_fire(local(2026, 9, 18, 18, 0)));
        assert!(!s.should_fire(local(2026, 9, 19, 18, 0)));
    }

    #[test]
    fn weekly_fires_only_on_its_named_day() {
        let s = schedule("weekly:mon", "09:00");
        // 2026-09-21 is a Monday, 2026-09-22 a Tuesday.
        assert!(s.should_fire(local(2026, 9, 21, 9, 30)));
        assert!(!s.should_fire(local(2026, 9, 22, 9, 30)));
    }

    #[test]
    fn fire_time_is_read_in_local_time_not_utc() {
        let s = schedule("daily", "17:30");
        assert!(!s.should_fire(local(2026, 9, 21, 17, 29)));
        assert!(s.should_fire(local(2026, 9, 21, 17, 31)));
    }

    #[test]
    fn a_recent_firing_suppresses_the_next_one() {
        let now = local(2026, 9, 21, 18, 0);
        let mut s = schedule("daily", "17:30");
        s.last_fired_at = Some(now.timestamp() - 60);
        assert!(!s.should_fire(now));
        s.last_fired_at = Some(now.timestamp() - crate::config::SCHEDULE_MIN_REFIRE_SECS - 1);
        assert!(s.should_fire(now));
    }

    #[test]
    fn paused_and_malformed_rows_never_fire() {
        let now = local(2026, 9, 21, 18, 0);
        let mut paused = schedule("daily", "17:30");
        paused.paused = true;
        assert!(!paused.should_fire(now));
        assert!(!schedule("daily", "half past five").should_fire(now));
    }
}

/// US equity/options data is settled on an Eastern-time clock, and this
/// is well after the 16:00 close and the 16:15 index close — late
/// enough that a session's history has been written upstream.
const SESSION_AVAILABLE_AFTER: NaiveTime = match NaiveTime::from_hms_opt(18, 0, 0) {
    Some(t) => t,
    None => unreachable!(),
};

/// The most recent trading session whose data a schedule can expect to
/// find, as of `now`.
///
/// A schedule fires on the user's own clock, but which session is
/// *available* is an Eastern-time question, so the two are resolved
/// separately. Firing at 17:30 local used to queue "local yesterday"
/// unconditionally: on a Tuesday evening that asked for Monday, so an
/// evening schedule ran a full day behind forever, and a schedule set
/// for Monday morning asked for Sunday and got nothing at all.
///
/// Weekends roll back to Friday. Market holidays are not modelled —
/// the server returns no rows for one, the task is recorded empty, and
/// the next fire moves on; encoding a holiday calendar here would age
/// badly for no gain.
pub fn last_available_session(now: DateTime<Utc>) -> Option<chrono::NaiveDate> {
    let et = now.with_timezone(&chrono_tz::America::New_York);
    let mut date = if et.time() >= SESSION_AVAILABLE_AFTER {
        et.date_naive()
    } else {
        et.date_naive().pred_opt()?
    };
    while matches!(date.weekday(), Weekday::Sat | Weekday::Sun) {
        date = date.pred_opt()?;
    }
    Some(date)
}

#[cfg(test)]
mod session_tests {
    use super::*;
    use chrono::TimeZone;

    fn et(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        chrono_tz::America::New_York
            .with_ymd_and_hms(y, m, d, h, min, 0)
            .single()
            .expect("unambiguous local time")
            .with_timezone(&Utc)
    }

    /// 2026-09-22 is a Tuesday.
    #[test]
    fn an_evening_fire_gets_todays_session() {
        assert_eq!(
            last_available_session(et(2026, 9, 22, 19, 30)),
            chrono::NaiveDate::from_ymd_opt(2026, 9, 22),
        );
    }

    #[test]
    fn a_fire_before_settlement_gets_the_previous_session() {
        // 09:00 ET Tuesday — Tuesday's session has not happened yet.
        assert_eq!(
            last_available_session(et(2026, 9, 22, 9, 0)),
            chrono::NaiveDate::from_ymd_opt(2026, 9, 21),
        );
    }

    /// The old behaviour's worst case: a Monday-morning schedule asked
    /// for Sunday, which never has data.
    #[test]
    fn a_monday_morning_fire_gets_friday_not_sunday() {
        let monday = et(2026, 9, 21, 8, 0);
        let got = last_available_session(monday).expect("a session exists");
        assert_eq!(got, chrono::NaiveDate::from_ymd_opt(2026, 9, 18).unwrap());
        assert_eq!(got.weekday(), Weekday::Fri);
    }

    #[test]
    fn a_weekend_fire_rolls_back_to_friday() {
        for (day, hour) in [(19u32, 20u32), (20, 20)] {
            let got = last_available_session(et(2026, 9, day, hour, 0)).expect("a session exists");
            assert_eq!(got.weekday(), Weekday::Fri, "day {day}");
        }
    }

    /// The boundary is Eastern, not local: the same instant resolves
    /// the same session wherever the machine's clock is set.
    #[test]
    fn the_boundary_is_eastern_regardless_of_machine_timezone() {
        // 17:59 ET is before the cutoff, 18:01 ET is after it.
        assert_eq!(
            last_available_session(et(2026, 9, 22, 17, 59)),
            chrono::NaiveDate::from_ymd_opt(2026, 9, 21),
        );
        assert_eq!(
            last_available_session(et(2026, 9, 22, 18, 1)),
            chrono::NaiveDate::from_ymd_opt(2026, 9, 22),
        );
    }
}

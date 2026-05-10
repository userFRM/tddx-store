//! Cron-style scheduled downloads, stored alongside the queue.
//!
//! A `Schedule` row says "every day at HH:MM ET, queue this DataSpec".
//! The desktop app's runtime (or `tdds-cli schedule run`) ticks
//! these and enqueues fresh tasks. We keep cron parsing minimal —
//! supports `daily HH:MM` and `weekday HH:MM` for now; richer cron
//! lives behind the optional cron-parser dep when we need it.

use chrono::{DateTime, NaiveTime, Utc};
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
    /// One of: "daily", "weekdays", "weekly:mon", …
    pub cron_kind: String,
    pub at_time: String, // "HH:MM"
    pub last_fired_at: Option<i64>,
    pub paused: bool,
    pub created_at: i64,
}

impl Schedule {
    pub fn at(&self) -> Option<NaiveTime> {
        NaiveTime::parse_from_str(&self.at_time, "%H:%M").ok()
    }

    /// Should this schedule fire `now`? Returns true if today's local
    /// fire-time has passed AND we haven't fired in the last 23 h.
    pub fn should_fire(&self, now: DateTime<Utc>) -> bool {
        if self.paused {
            return false;
        }
        let Some(time) = self.at() else {
            return false;
        };
        let local = now.naive_utc().date().and_time(time);
        if now.naive_utc() < local {
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

//! Centralized constants. Audit findings flagged scattered magic numbers
//! (pool size, retry sleep, schedule cooldown, polling intervals). One
//! place to change them, one place to grep for "what's the default".

/// SQLite pool size for the queue DB. Sized above the absolute worst-case
/// worker count (Pro stock + Pro option = 8 + 8 = 16) so polling commands
/// (snapshot, schedule_list) can always grab a connection while every
/// worker is mid-`claim_next_by_class`. Without this the pool saturates
/// under load and snapshots queue forever.
pub const SQLITE_POOL_SIZE: u32 = 20;

/// Backoff sleep after a `claim_next` SQL error. Workers retry until
/// the queue is reachable again rather than dying.
pub const CLAIM_RETRY_MS: u64 = 500;

/// Seconds between worker lease heartbeats while a task is `running`.
pub const TASK_HEARTBEAT_INTERVAL_SECS: u64 = 5;

/// A `running` row is stale once it misses two heartbeat intervals.
pub const TASK_HEARTBEAT_STALE_AFTER_SECS: i64 = (TASK_HEARTBEAT_INTERVAL_SECS as i64) * 2;

/// Min seconds between two firings of the same schedule. 23 h gives
/// daylight-savings safety on the weekday/daily rules without a strict
/// cron parser.
pub const SCHEDULE_MIN_REFIRE_SECS: i64 = 23 * 3600;

/// Activity-log ring buffer cap (per process).
pub const MAX_LOG_ENTRIES: usize = 1000;

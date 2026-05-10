//! SQLite-backed task queue. Survives restarts — pull pending → claim →
//! mark done / failed. The schema is small on purpose: the queue is
//! work-list state, not the data itself.

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

use crate::config::SQLITE_POOL_SIZE;
use crate::format::OutputFormat;
use crate::spec::{DataKind, DataSpec};
use crate::tier::AssetClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Done,
    Failed,
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub spec: DataSpec,
    pub format: OutputFormat,
    pub output_dir: String,
    pub status: TaskStatus,
    pub priority: i32,
    pub attempts: i32,
    pub error: Option<String>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
    pub rows: Option<i64>,
    pub bytes: Option<i64>,
}

#[derive(Clone)]
pub struct Queue {
    pool: SqlitePool,
    owner_id: String,
}

impl Queue {
    pub async fn open(path: &Path) -> crate::Result<Self> {
        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .map_err(|e| crate::Error::Other(format!("sqlite uri: {e}")))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(SQLITE_POOL_SIZE)
            .connect_with(opts)
            .await?;

        Self::from_pool_with_owner(pool, uuid::Uuid::new_v4().to_string()).await
    }

    async fn from_pool_with_owner(pool: SqlitePool, owner_id: String) -> crate::Result<Self> {
        Self::init_schema(&pool).await?;
        Ok(Self { pool, owner_id })
    }

    async fn init_schema(pool: &SqlitePool) -> crate::Result<()> {
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS tasks (
                id          TEXT PRIMARY KEY,
                kind        TEXT NOT NULL,
                symbol      TEXT NOT NULL,
                date        TEXT NOT NULL,
                interval    TEXT,
                expiration  TEXT NOT NULL DEFAULT '*',
                strike      TEXT NOT NULL DEFAULT '*',
                right_      TEXT NOT NULL DEFAULT 'both',
                format      TEXT NOT NULL,
                output_dir  TEXT NOT NULL,
                status      TEXT NOT NULL,
                priority    INTEGER NOT NULL DEFAULT 0,
                attempts    INTEGER NOT NULL DEFAULT 0,
                error       TEXT,
                created_at  INTEGER NOT NULL,
                finished_at INTEGER,
                rows        INTEGER,
                bytes       INTEGER,
                transforms_json   TEXT,
                claimed_by        TEXT,
                claimed_at        INTEGER,
                last_heartbeat_at INTEGER
            )"#,
        )
        .execute(pool)
        .await?;

        // Live-migrate older databases. Tolerate ONLY duplicate-column
        // errors so idempotent re-opens succeed; any other failure
        // (locked DB, corruption, permission denial) propagates.
        Self::add_column_if_missing(pool, "ALTER TABLE tasks ADD COLUMN transforms_json TEXT")
            .await?;
        Self::add_column_if_missing(pool, "ALTER TABLE tasks ADD COLUMN claimed_by TEXT").await?;
        Self::add_column_if_missing(pool, "ALTER TABLE tasks ADD COLUMN claimed_at INTEGER")
            .await?;
        Self::add_column_if_missing(
            pool,
            "ALTER TABLE tasks ADD COLUMN last_heartbeat_at INTEGER",
        )
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS tasks_status_priority ON tasks(status, priority DESC, created_at)",
        )
        .execute(pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS tasks_status_heartbeat ON tasks(status, last_heartbeat_at)",
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    async fn add_column_if_missing(pool: &SqlitePool, sql: &str) -> crate::Result<()> {
        if let Err(e) = sqlx::query(sql).execute(pool).await {
            if !Self::is_duplicate_column_error(&e) {
                return Err(e.into());
            }
        }
        Ok(())
    }

    fn is_duplicate_column_error(err: &sqlx::Error) -> bool {
        err.to_string()
            .to_ascii_lowercase()
            .contains("duplicate column name")
    }

    pub async fn enqueue(
        &self,
        spec: DataSpec,
        format: OutputFormat,
        output_dir: &str,
        priority: i32,
    ) -> crate::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let transforms_json = if spec.transforms.is_noop() {
            None
        } else {
            Some(serde_json::to_string(&spec.transforms)?)
        };
        sqlx::query(
            r#"INSERT INTO tasks (id, kind, symbol, date, interval, expiration, strike, right_,
                format, output_dir, status, priority, attempts, created_at, transforms_json)
               VALUES (?,?,?,?,?,?,?,?,?,?,?,?,0,?,?)"#,
        )
        .bind(&id)
        .bind(spec.kind.as_str())
        .bind(&spec.symbol)
        .bind(spec.ymd())
        .bind(&spec.interval)
        .bind(&spec.expiration)
        .bind(&spec.strike)
        .bind(&spec.right)
        .bind(format.extension())
        .bind(output_dir)
        .bind("pending")
        .bind(priority)
        .bind(now)
        .bind(transforms_json)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// Atomically claim the next pending task by flipping it to `running`.
    /// Returns `Ok(None)` when the queue is empty.
    ///
    /// Two workers can race here — without atomicity both can SELECT the
    /// same row, both UPDATE it, and both run the same task against the
    /// same output path. We solve this with a single
    /// `UPDATE … WHERE id = (SELECT …) AND status='pending' RETURNING …`
    /// — SQLite (≥3.35) executes the SELECT and the conditional UPDATE
    /// in one statement, guaranteed atomic at the page level. A loser
    /// of the race gets `None`, triggering a retry on the next outer
    /// loop tick.
    pub async fn claim_next(&self) -> crate::Result<Option<Task>> {
        let now = Utc::now().timestamp();
        let row: Option<TaskRow> = sqlx::query_as(
            r#"
            UPDATE tasks
               SET status = 'running',
                   attempts = attempts + 1,
                   claimed_by = ?,
                   claimed_at = ?,
                   last_heartbeat_at = ?,
                   finished_at = NULL
             WHERE id = (
                 SELECT id FROM tasks
                  WHERE status = 'pending'
                  ORDER BY priority DESC, created_at ASC
                  LIMIT 1
             )
               AND status = 'pending'
            RETURNING *
            "#,
        )
        .bind(&self.owner_id)
        .bind(now)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        row.try_into().map(Some)
    }

    /// Atomic claim restricted to one asset class. Used by the
    /// per-class worker pools so a Pro-Options run can't starve
    /// Standard-Stocks workers (and vice versa) — each class drains
    /// at its own server-permitted concurrency.
    ///
    /// Index/Rate classes have no `DataKind` variants today, so they
    /// short-circuit to `None` rather than burning a SQL round-trip.
    pub async fn claim_next_by_class(&self, class: AssetClass) -> crate::Result<Option<Task>> {
        let kinds: &[&str] = match class {
            AssetClass::Stock => &["stock_trade", "stock_quote", "stock_trade_quote"],
            AssetClass::Option => &[
                "option_trade",
                "option_quote",
                "option_trade_quote",
                "option_oi",
            ],
            AssetClass::Index | AssetClass::Rate => return Ok(None),
        };
        let now = Utc::now().timestamp();
        let placeholders = vec!["?"; kinds.len()].join(",");
        let sql = format!(
            r#"
            UPDATE tasks
               SET status = 'running',
                   attempts = attempts + 1,
                   claimed_by = ?,
                   claimed_at = ?,
                   last_heartbeat_at = ?,
                   finished_at = NULL
             WHERE id = (
                 SELECT id FROM tasks
                  WHERE status = 'pending'
                    AND kind IN ({placeholders})
                  ORDER BY priority DESC, created_at ASC
                  LIMIT 1
             )
               AND status = 'pending'
            RETURNING *
            "#
        );
        let mut q = sqlx::query_as::<_, TaskRow>(&sql)
            .bind(&self.owner_id)
            .bind(now)
            .bind(now);
        for k in kinds {
            q = q.bind(*k);
        }
        let row: Option<TaskRow> = q.fetch_optional(&self.pool).await?;
        let Some(row) = row else {
            return Ok(None);
        };
        row.try_into().map(Some)
    }

    pub async fn fetch(&self, id: &str) -> crate::Result<Task> {
        let row: TaskRow = sqlx::query_as("SELECT * FROM tasks WHERE id=?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        row.try_into()
    }

    pub async fn list(&self, status: Option<TaskStatus>, limit: i64) -> crate::Result<Vec<Task>> {
        let rows: Vec<TaskRow> = match status {
            Some(s) => sqlx::query_as(
                "SELECT * FROM tasks WHERE status=? ORDER BY priority DESC, created_at ASC LIMIT ?",
            )
            .bind(status_to_str(s))
            .bind(limit)
            .fetch_all(&self.pool)
            .await?,
            None => {
                sqlx::query_as("SELECT * FROM tasks ORDER BY priority DESC, created_at ASC LIMIT ?")
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// Mark a task `done`. Conditional on `status='running'` so a row
    /// that was cancelled mid-flight (status flipped to `failed` by
    /// `cancel()`) doesn't get clobbered back to `done` by the worker
    /// finishing its in-flight call. Returns `true` if the row was
    /// updated; `false` means the row was already in a terminal state
    /// (probably cancelled) and the caller should bail out — the
    /// worker checks this and skips its on-disk file rename.
    pub async fn mark_done(&self, id: &str, rows: i64, bytes: i64) -> crate::Result<bool> {
        let r = sqlx::query(
            "UPDATE tasks SET status='done', rows=?, bytes=?, finished_at=?, \
             claimed_by=NULL, claimed_at=NULL, last_heartbeat_at=NULL \
             WHERE id=? AND status='running' AND claimed_by=?",
        )
        .bind(rows)
        .bind(bytes)
        .bind(Utc::now().timestamp())
        .bind(id)
        .bind(&self.owner_id)
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected() == 1)
    }

    pub async fn mark_empty(&self, id: &str) -> crate::Result<bool> {
        let r = sqlx::query(
            "UPDATE tasks SET status='empty', finished_at=?, \
             claimed_by=NULL, claimed_at=NULL, last_heartbeat_at=NULL \
             WHERE id=? AND status='running' AND claimed_by=?",
        )
        .bind(Utc::now().timestamp())
        .bind(id)
        .bind(&self.owner_id)
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected() == 1)
    }

    pub async fn mark_failed(&self, id: &str, err: &str) -> crate::Result<bool> {
        let r = sqlx::query(
            "UPDATE tasks SET status='failed', error=?, finished_at=?, \
             claimed_by=NULL, claimed_at=NULL, last_heartbeat_at=NULL \
             WHERE id=? AND status='running' AND claimed_by=?",
        )
        .bind(err)
        .bind(Utc::now().timestamp())
        .bind(id)
        .bind(&self.owner_id)
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected() == 1)
    }

    /// Update the current process' lease on a `running` row. Returns
    /// false once the row is no longer both `running` and owned by this
    /// queue instance (cancelled, reaped, or stolen by another process).
    pub async fn heartbeat(&self, id: &str) -> crate::Result<bool> {
        let r = sqlx::query(
            "UPDATE tasks SET last_heartbeat_at=? \
             WHERE id=? AND status='running' AND claimed_by=?",
        )
        .bind(Utc::now().timestamp())
        .bind(id)
        .bind(&self.owner_id)
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected() == 1)
    }

    /// Sweep: only `running` rows with stale or missing heartbeats are
    /// demoted back to `pending`. Done in an IMMEDIATE transaction so a
    /// fresh heartbeat can't interleave between the stale check and the
    /// reset update.
    pub async fn reset_running_to_pending(&self) -> crate::Result<usize> {
        let stale_before = Utc::now().timestamp() - crate::config::TASK_HEARTBEAT_STALE_AFTER_SECS;
        let mut conn = self.pool.acquire().await?;
        sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;
        let result = sqlx::query(
            "UPDATE tasks
                SET status='pending',
                    error=NULL,
                    finished_at=NULL,
                    rows=NULL,
                    bytes=NULL,
                    claimed_by=NULL,
                    claimed_at=NULL,
                    last_heartbeat_at=NULL
              WHERE status='running'
                AND (last_heartbeat_at IS NULL OR last_heartbeat_at < ?)",
        )
        .bind(stale_before)
        .execute(&mut *conn)
        .await;
        match result {
            Ok(result) => {
                sqlx::query("COMMIT").execute(&mut *conn).await?;
                Ok(result.rows_affected() as usize)
            }
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e.into())
            }
        }
    }

    /// Mark a pending or running task as cancelled. A running worker
    /// keeps executing until its current upstream call returns, but its
    /// next heartbeat and final mark_* call will observe that the row is
    /// no longer `running`.
    pub async fn cancel(&self, id: &str) -> crate::Result<()> {
        sqlx::query(
            "UPDATE tasks SET status='failed', error='cancelled by user', finished_at=?, \
             claimed_by=NULL, claimed_at=NULL, last_heartbeat_at=NULL \
             WHERE id=? AND status IN ('pending','running')",
        )
        .bind(Utc::now().timestamp())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn requeue(&self, id: &str) -> crate::Result<()> {
        sqlx::query(
            "UPDATE tasks
                SET status='pending',
                    error=NULL,
                    finished_at=NULL,
                    rows=NULL,
                    bytes=NULL,
                    claimed_by=NULL,
                    claimed_at=NULL,
                    last_heartbeat_at=NULL
              WHERE id=?",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn counts(&self) -> crate::Result<Vec<(TaskStatus, i64)>> {
        let rows: Vec<(String, i64)> =
            sqlx::query_as("SELECT status, COUNT(*) FROM tasks GROUP BY status")
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .filter_map(|(s, n)| status_from_str(&s).map(|st| (st, n)))
            .map(Ok)
            .collect()
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

fn status_to_str(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Pending => "pending",
        TaskStatus::Running => "running",
        TaskStatus::Done => "done",
        TaskStatus::Failed => "failed",
        TaskStatus::Empty => "empty",
    }
}
fn status_from_str(s: &str) -> Option<TaskStatus> {
    Some(match s {
        "pending" => TaskStatus::Pending,
        "running" => TaskStatus::Running,
        "done" => TaskStatus::Done,
        "failed" => TaskStatus::Failed,
        "empty" => TaskStatus::Empty,
        _ => return None,
    })
}

#[derive(sqlx::FromRow)]
struct TaskRow {
    id: String,
    kind: String,
    symbol: String,
    date: String,
    interval: Option<String>,
    expiration: String,
    strike: String,
    right_: String,
    format: String,
    output_dir: String,
    status: String,
    priority: i32,
    attempts: i32,
    error: Option<String>,
    created_at: i64,
    finished_at: Option<i64>,
    rows: Option<i64>,
    bytes: Option<i64>,
    /// JSON-serialised `Transforms` blob. Optional column added in v0.1.1.
    #[sqlx(default)]
    transforms_json: Option<String>,
}

impl TryFrom<TaskRow> for Task {
    type Error = crate::Error;
    fn try_from(r: TaskRow) -> std::result::Result<Self, Self::Error> {
        let kind = DataKind::parse(&r.kind)
            .ok_or_else(|| crate::Error::Other(format!("bad kind {}", r.kind)))?;
        let date = NaiveDate::parse_from_str(&r.date, "%Y%m%d")
            .map_err(|e| crate::Error::Other(format!("bad date {}: {}", r.date, e)))?;
        let format = OutputFormat::parse(&r.format)
            .ok_or_else(|| crate::Error::Other(format!("bad format {}", r.format)))?;
        let status = status_from_str(&r.status)
            .ok_or_else(|| crate::Error::Other(format!("bad status {}", r.status)))?;
        Ok(Task {
            id: r.id,
            spec: DataSpec {
                kind,
                symbol: r.symbol,
                date,
                interval: r.interval,
                expiration: r.expiration,
                strike: r.strike,
                right: r.right_,
                transforms: r
                    .transforms_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default(),
            },
            format,
            output_dir: r.output_dir,
            status,
            priority: r.priority,
            attempts: r.attempts,
            error: r.error,
            created_at: r.created_at,
            finished_at: r.finished_at,
            rows: r.rows,
            bytes: r.bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sqlx::Row;

    fn sample_spec() -> DataSpec {
        DataSpec {
            kind: DataKind::StockTrade,
            symbol: "AAPL".into(),
            date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
            interval: None,
            expiration: "*".into(),
            strike: "*".into(),
            right: "both".into(),
            transforms: crate::Transforms::default(),
        }
    }

    async fn single_connection_memory_queue(owner_id: &str) -> Queue {
        // `:memory:` is per-connection in SQLite, so pin tests that don't
        // need true parallel DB access to one pooled connection.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(":memory:")
            .await
            .unwrap();
        Queue::from_pool_with_owner(pool, owner_id.to_string())
            .await
            .unwrap()
    }

    async fn shared_memory_pool(max_connections: u32) -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(max_connections)
            .connect(&format!(
                "sqlite:file:{}?mode=memory&cache=shared",
                uuid::Uuid::new_v4()
            ))
            .await
            .unwrap()
    }

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tdds-queue-tests-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("queue.db")
    }

    async fn create_pool_for_path(path: &Path) -> SqlitePool {
        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .unwrap()
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn claim_next_is_atomic() {
        let pool = shared_memory_pool(8).await;
        let queue = Queue::from_pool_with_owner(pool.clone(), "owner-a".into())
            .await
            .unwrap();
        let id = queue
            .enqueue(sample_spec(), OutputFormat::Parquet, "/tmp", 0)
            .await
            .unwrap();
        let queue = std::sync::Arc::new(queue);
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(17));
        let mut joins = Vec::new();
        for _ in 0..16 {
            let queue = queue.clone();
            let barrier = barrier.clone();
            joins.push(tokio::spawn(async move {
                barrier.wait().await;
                queue.claim_next().await.unwrap().map(|task| task.id)
            }));
        }
        barrier.wait().await;

        let mut claimed = Vec::new();
        for join in joins {
            if let Some(task_id) = join.await.unwrap() {
                claimed.push(task_id);
            }
        }

        assert_eq!(claimed, vec![id.clone()]);
        let task = queue.fetch(&id).await.unwrap();
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.attempts, 1);
        let row =
            sqlx::query("SELECT claimed_by, claimed_at, last_heartbeat_at FROM tasks WHERE id=?")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row.get::<String, _>(0), queue.owner_id);
        assert!(row.get::<i64, _>(1) > 0);
        assert!(row.get::<i64, _>(2) > 0);
    }

    #[tokio::test]
    async fn cancel_while_running_keeps_failed_terminal_state() {
        let queue = single_connection_memory_queue("owner-a").await;
        let id = queue
            .enqueue(sample_spec(), OutputFormat::Parquet, "/tmp", 0)
            .await
            .unwrap();
        let claimed = queue.claim_next().await.unwrap().unwrap();
        assert_eq!(claimed.id, id);

        queue.cancel(&id).await.unwrap();

        assert!(!queue.heartbeat(&id).await.unwrap());
        assert!(!queue.mark_done(&id, 42, 2048).await.unwrap());
        assert!(!queue.mark_empty(&id).await.unwrap());
        let task = queue.fetch(&id).await.unwrap();
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.error.as_deref(), Some("cancelled by user"));
        assert!(task.finished_at.is_some());
        assert_eq!(task.rows, None);
        assert_eq!(task.bytes, None);
    }

    #[tokio::test]
    async fn reset_running_to_pending_reaps_only_stale_leases() {
        let pool = shared_memory_pool(8).await;
        let owner_a = Queue::from_pool_with_owner(pool.clone(), "owner-a".into())
            .await
            .unwrap();
        let owner_b = Queue::from_pool_with_owner(pool.clone(), "owner-b".into())
            .await
            .unwrap();
        let id = owner_a
            .enqueue(sample_spec(), OutputFormat::Parquet, "/tmp", 0)
            .await
            .unwrap();
        owner_a.claim_next().await.unwrap().unwrap();

        assert!(owner_a.heartbeat(&id).await.unwrap());
        assert_eq!(owner_b.reset_running_to_pending().await.unwrap(), 0);

        let stale = Utc::now().timestamp() - crate::config::TASK_HEARTBEAT_STALE_AFTER_SECS - 1;
        sqlx::query("UPDATE tasks SET last_heartbeat_at=? WHERE id=?")
            .bind(stale)
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(owner_b.reset_running_to_pending().await.unwrap(), 1);
        assert!(!owner_a.mark_done(&id, 7, 64).await.unwrap());

        let task = owner_b.fetch(&id).await.unwrap();
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.error, None);
        assert_eq!(task.rows, None);
        assert_eq!(task.bytes, None);

        let reclaimed = owner_b.claim_next().await.unwrap().unwrap();
        assert_eq!(reclaimed.id, id);
        let row = sqlx::query("SELECT claimed_by FROM tasks WHERE id=?")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>(0), owner_b.owner_id);
    }

    #[tokio::test]
    async fn open_tolerates_duplicate_column_migrations() {
        let path = temp_db_path("duplicate-columns");
        let pool = create_pool_for_path(&path).await;
        sqlx::query(
            r#"CREATE TABLE tasks (
                id                TEXT PRIMARY KEY,
                kind              TEXT NOT NULL,
                symbol            TEXT NOT NULL,
                date              TEXT NOT NULL,
                interval          TEXT,
                expiration        TEXT NOT NULL DEFAULT '*',
                strike            TEXT NOT NULL DEFAULT '*',
                right_            TEXT NOT NULL DEFAULT 'both',
                format            TEXT NOT NULL,
                output_dir        TEXT NOT NULL,
                status            TEXT NOT NULL,
                priority          INTEGER NOT NULL DEFAULT 0,
                attempts          INTEGER NOT NULL DEFAULT 0,
                error             TEXT,
                created_at        INTEGER NOT NULL,
                finished_at       INTEGER,
                rows              INTEGER,
                bytes             INTEGER,
                transforms_json   TEXT,
                claimed_by        TEXT,
                claimed_at        INTEGER,
                last_heartbeat_at INTEGER
            )"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let queue = Queue::open(&path).await.unwrap();
        let id = queue
            .enqueue(sample_spec(), OutputFormat::Parquet, "/tmp", 0)
            .await
            .unwrap();
        let claimed = queue.claim_next().await.unwrap().unwrap();
        assert_eq!(claimed.id, id);

        drop(queue);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn open_propagates_non_duplicate_migration_errors() {
        let path = temp_db_path("view-schema");
        let pool = create_pool_for_path(&path).await;
        sqlx::query("CREATE VIEW tasks AS SELECT 'id' AS id")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;

        let err = match Queue::open(&path).await {
            Ok(_) => {
                panic!("Queue::open unexpectedly succeeded against a view-backed tasks object")
            }
            Err(err) => err,
        };
        let msg = err.to_string().to_ascii_lowercase();
        assert!(msg.contains("view") || msg.contains("cannot add a column"));

        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}

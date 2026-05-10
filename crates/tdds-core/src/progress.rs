//! Worker-emitted progress events for the UI / CLI / MCP to consume.

use crate::spec::DataKind;
use chrono::NaiveDate;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ProgressEvent {
    Queued {
        task_id: String,
        kind: DataKind,
        symbol: String,
        date: NaiveDate,
    },
    Started {
        task_id: String,
    },
    Done {
        task_id: String,
        rows: u64,
        bytes: u64,
        millis: u64,
    },
    Empty {
        task_id: String,
        millis: u64,
    },
    Failed {
        task_id: String,
        error: String,
        millis: u64,
    },
    /// Whole-pool snapshot for ETA / dashboard.
    Pool {
        running: usize,
        queued: usize,
        completed: u64,
        failed: u64,
        bytes_written: u64,
        rows_written: u64,
        wall_ms: u64,
    },
}

#[derive(Debug, Default, Clone)]
pub struct Progress {
    pub running: usize,
    pub queued: usize,
    pub completed: u64,
    pub failed: u64,
    pub bytes_written: u64,
    pub rows_written: u64,
    pub started_at: Option<std::time::Instant>,
}

impl Progress {
    pub fn new() -> Self {
        Self {
            started_at: Some(std::time::Instant::now()),
            ..Default::default()
        }
    }

    pub fn wall_ms(&self) -> u64 {
        self.started_at
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn snapshot_event(&self) -> ProgressEvent {
        ProgressEvent::Pool {
            running: self.running,
            queued: self.queued,
            completed: self.completed,
            failed: self.failed,
            bytes_written: self.bytes_written,
            rows_written: self.rows_written,
            wall_ms: self.wall_ms(),
        }
    }

    /// Estimate finish time. Returns ms remaining or None when no signal yet.
    pub fn eta_ms(&self) -> Option<u64> {
        if self.completed == 0 || self.queued == 0 {
            return None;
        }
        let avg_ms_per_task = self.wall_ms() as f64 / self.completed as f64;
        Some((avg_ms_per_task * self.queued as f64) as u64)
    }
}

//! tdds-core — engine for the TdDx Store desktop app + CLI.
//!
//! Pieces:
//!   spec      — DataKind / DataSpec describing what to fetch.
//!   client    — wraps thetadatadx::ThetaDataClient with a creds loader.
//!   queue     — sqlite-backed task queue (resumable across restarts).
//!   worker    — async worker pool driving downloads.
//!   format    — Arrow → Parquet / CSV / JSON / JSONL writers.
//!   coverage  — what's already on disk for a given DataSpec.
//!   progress  — typed progress events emitted by workers.

pub mod client;
pub mod config;
pub mod coverage;
pub mod format;
pub mod preview;
pub mod progress;
pub mod queue;
pub mod registry;
pub mod schedule;
pub mod spec;
pub mod tier;
pub mod transform;
pub mod worker;
pub mod yaml_meta;

pub use client::Client;
pub use coverage::Coverage;
pub use format::OutputFormat;
pub use preview::{preview as preview_parquet, PreviewField, PreviewResult};
pub use progress::{Progress, ProgressEvent};
pub use queue::{Queue, Task, TaskStatus};
pub use registry::{
    all_endpoints, dispatch_raw, dispatch_to_arrow, dispatch_to_file, endpoints_by_category,
    find_endpoint, EndpointInfo, ParamInfo,
};
pub use schedule::Schedule;
pub use spec::{DataKind, DataSpec, EndpointSpec};
pub use tier::{
    evaluate as tier_evaluate, governing_tier, is_tier_denied, min_tier_for, Tier, TierVerdict,
    UserTiers, UPGRADE_URL,
};
pub use transform::Transforms;
pub use worker::Pool;
pub use yaml_meta::{catalogue as endpoint_catalogue, endpoint_meta, EndpointMeta};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("thetadatadx: {0}")]
    Theta(#[from] thetadatadx::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("arrow: {0}")]
    Arrow(#[from] arrow_schema::ArrowError),
    #[error("parquet: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),
    #[error("serde_json: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

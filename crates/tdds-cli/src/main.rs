//! tddl — CLI on top of tdds-core. Five verbs:
//!
//!   add     queue a single (kind, symbol, date) task or a date range
//!   list    show queued / running / done / failed tasks
//!   run     start workers and drain the queue
//!   status  one-shot snapshot of queue counts + bytes / rows
//!   view    coverage report for a dataset on disk

use std::path::PathBuf;

use chrono::NaiveDate;
use clap::{Parser, Subcommand};
use tdds_core::{
    coverage, format::OutputFormat, queue::TaskStatus, Client, DataKind, DataSpec, Pool, Queue,
};
use tokio::sync::mpsc;

#[derive(Parser, Debug)]
#[command(version, about = "ThetaData downloader / queue manager")]
struct Cli {
    /// Path to credentials file. Defaults to $TDDS_CREDS or ./creds.txt.
    #[arg(long, env = "TDDS_CREDS", global = true)]
    creds: Option<PathBuf>,

    /// SQLite queue path.
    #[arg(long, env = "TDDS_DB", default_value = "tddx-store.db", global = true)]
    db: PathBuf,

    /// Output root for downloaded files (one subdir per kind).
    #[arg(long, env = "TDDS_OUT", default_value = "./tdds_data", global = true)]
    out: PathBuf,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Queue task(s).
    Add {
        /// Kind: stock_trade | stock_quote | stock_trade_quote | option_trade | option_quote | option_trade_quote | option_oi
        #[arg(long)]
        kind: String,
        #[arg(long)]
        symbol: String,
        /// YYYYMMDD or YYYY-MM-DD.
        #[arg(long)]
        date: Option<String>,
        /// Inclusive start date for a range (use with --end).
        #[arg(long)]
        start: Option<String>,
        /// Inclusive end date for a range.
        #[arg(long)]
        end: Option<String>,
        #[arg(long, default_value = "parquet")]
        format: String,
        /// Quote interval: "0" = tick-by-tick, "1s" = sampled.
        #[arg(long)]
        interval: Option<String>,
        #[arg(long, default_value = "*")]
        expiration: String,
        #[arg(long, default_value = "*")]
        strike: String,
        #[arg(long, default_value = "both")]
        right: String,
        #[arg(long, default_value_t = 0)]
        priority: i32,
    },
    /// List tasks (any status by default).
    List {
        #[arg(long)]
        status: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
    /// Drain the queue. Concurrency is fixed by the user's per-class
    /// ThetaData subscription tier — Pro Stocks gets 8 stock workers,
    /// Standard Options gets 4 option workers, etc. There is no flag to
    /// override; over-provisioning beyond `2^tier` is silently 429'd /
    /// queued by FPSS and just hurts latency.
    Run {
        /// Print one-line progress every N tasks completed.
        #[arg(long, default_value_t = 1)]
        report_every: u64,
    },
    /// Snapshot of queue counts + on-disk bytes.
    Status,
    /// Per-(kind, symbol) coverage report from on-disk files.
    View {
        #[arg(long)]
        symbol: Option<String>,
        #[arg(long)]
        kind: Option<String>,
    },
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .compact()
        .init();

    let cli = Cli::parse();
    std::fs::create_dir_all(&cli.out)?;
    if let Some(parent) = cli.db.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let queue = Queue::open(&cli.db).await?;

    match cli.cmd {
        Cmd::Add {
            kind,
            symbol,
            date,
            start,
            end,
            format,
            interval,
            expiration,
            strike,
            right,
            priority,
        } => {
            let kind =
                DataKind::parse(&kind).ok_or_else(|| anyhow::anyhow!("unknown kind '{kind}'"))?;
            let format = OutputFormat::parse(&format)
                .ok_or_else(|| anyhow::anyhow!("unknown format '{format}'"))?;
            let dates: Vec<NaiveDate> = match (date, start, end) {
                (Some(d), _, _) => vec![parse_ymd(&d)?],
                (None, Some(s), Some(e)) => {
                    // Resolve actual trading days from the server.
                    let client = Client::connect(cli.creds.as_deref()).await?;
                    let s = parse_ymd(&s)?;
                    let e = parse_ymd(&e)?;
                    client.trading_days(&symbol, s, e).await?
                }
                _ => anyhow::bail!("pass --date or --start + --end"),
            };
            for d in dates {
                let spec = DataSpec {
                    kind,
                    symbol: symbol.clone(),
                    date: d,
                    interval: interval.clone(),
                    expiration: expiration.clone(),
                    strike: strike.clone(),
                    right: right.clone(),
                    transforms: Default::default(),
                };
                let id = queue
                    .enqueue(spec, format, &cli.out.to_string_lossy(), priority)
                    .await?;
                println!("queued {id}  {symbol} {} {}", kind.as_str(), d);
            }
        }

        Cmd::List { status, limit } => {
            let st = status.as_deref().and_then(parse_status);
            let tasks = queue.list(st, limit).await?;
            println!(
                "{:<36}  {:<7}  {:<18}  {:<7}  {:<10}  {:>9}  error",
                "id", "status", "kind", "symbol", "date", "rows"
            );
            for t in &tasks {
                let date_str = t.spec.date.format("%Y-%m-%d").to_string();
                let rows = t.rows.map(|n| n.to_string()).unwrap_or_default();
                let err = t.error.as_deref().unwrap_or("");
                println!(
                    "{:<36}  {:<7}  {:<18}  {:<7}  {:<10}  {:>9}  {}",
                    t.id,
                    fmt_status(t.status),
                    t.spec.kind.as_str(),
                    t.spec.symbol,
                    date_str,
                    rows,
                    err
                );
            }
            println!("({} tasks)", tasks.len());
        }

        Cmd::Run { report_every } => {
            let client = Client::connect(cli.creds.as_deref()).await?;
            let tiers = client.user_tiers();
            let (tx, mut rx) = mpsc::channel(2048);
            let pool = Pool::new(client, queue.clone(), tiers).with_events(tx);
            let progress = pool.progress();
            let queue_for_count = queue.clone();
            let progress_handle = tokio::spawn(async move {
                let mut completed_seen = 0u64;
                while let Some(ev) = rx.recv().await {
                    if let tdds_core::ProgressEvent::Done { rows, bytes, .. } = &ev {
                        let p = progress.lock().await.clone();
                        completed_seen += 1;
                        if completed_seen.is_multiple_of(report_every) {
                            let queued = queue_for_count
                                .counts()
                                .await
                                .ok()
                                .and_then(|c| {
                                    c.into_iter()
                                        .find(|(s, _)| matches!(s, TaskStatus::Pending))
                                        .map(|(_, n)| n)
                                })
                                .unwrap_or(0);
                            let eta = p
                                .eta_ms()
                                .map(|ms| format!("{}m", ms / 60_000))
                                .unwrap_or_else(|| "—".into());
                            println!(
                                "[{:>5}m]  done={:>6}  rows={:>10}  MB={:>7.1}  +{rows} rows / {bytes} bytes  pending={queued}  eta={eta}",
                                p.wall_ms() / 60_000,
                                p.completed,
                                p.rows_written,
                                p.bytes_written as f64 / 1.0e6,
                            );
                        }
                    }
                }
            });
            pool.run().await?;
            drop(progress_handle.abort_handle());
            println!("queue drained");
        }

        Cmd::Status => {
            let counts = queue.counts().await?;
            for (s, n) in counts {
                println!("{:<8}  {n}", fmt_status(s));
            }
            let cov = coverage::scan(&cli.out)?;
            let total_bytes: u64 = cov.iter().map(|c| c.bytes).sum();
            let total_files: usize = cov.iter().map(|c| c.dates.len()).sum();
            println!(
                "datasets: {} groups, {} files, {:.1} MB",
                cov.len(),
                total_files,
                total_bytes as f64 / 1.0e6
            );
        }

        Cmd::View { symbol, kind } => {
            let cov = coverage::scan(&cli.out)?;
            let kind_filter = kind.as_deref().and_then(DataKind::parse);
            for c in cov {
                if let Some(k) = kind_filter {
                    if c.kind != k {
                        continue;
                    }
                }
                if let Some(s) = &symbol {
                    if !c.symbol.eq_ignore_ascii_case(s) {
                        continue;
                    }
                }
                let span = match (c.dates.first(), c.dates.last()) {
                    (Some(f), Some(l)) => format!("{}..{}", f, l),
                    _ => "—".into(),
                };
                println!(
                    "{:<8}  {:<18}  {:>4} files  {:>7.1} MB  {}",
                    c.symbol,
                    c.kind.as_str(),
                    c.dates.len(),
                    c.bytes as f64 / 1.0e6,
                    span
                );
            }
        }
    }
    Ok(())
}

fn parse_ymd(s: &str) -> anyhow::Result<NaiveDate> {
    let t = s.replace('-', "");
    NaiveDate::parse_from_str(&t, "%Y%m%d").map_err(|e| anyhow::anyhow!("bad date '{s}': {e}"))
}

fn parse_status(s: &str) -> Option<TaskStatus> {
    Some(match s {
        "pending" => TaskStatus::Pending,
        "running" => TaskStatus::Running,
        "done" => TaskStatus::Done,
        "failed" => TaskStatus::Failed,
        "empty" => TaskStatus::Empty,
        _ => return None,
    })
}

fn fmt_status(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Pending => "pending",
        TaskStatus::Running => "running",
        TaskStatus::Done => "done",
        TaskStatus::Failed => "failed",
        TaskStatus::Empty => "empty",
    }
}

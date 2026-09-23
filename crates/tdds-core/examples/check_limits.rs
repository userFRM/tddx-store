//! Verify the server constraints the app hides are still the ones it
//! was built against. Exits non-zero on any drift, in either direction.
//!
//! The constants in `spec.rs` — `MAX_RANGE_DAYS`,
//! `REJECTS_EXPIRATION_WILDCARD`, `REJECTS_TICK_INTERVAL` — are not in
//! the registry, the OpenAPI spec or the SDK types. They were found by
//! asking the server. Nothing tells us when they stop being true, and
//! the failure is quiet and bad: tighten a cap upstream and the app
//! cheerfully queues work that now always fails.
//!
//! Each check is one small, deliberately-shaped request, so this is
//! cheap: a few dozen calls, most of them rejected before any data
//! moves.
//!
//! **This signs in.** ThetaData keeps one live session per account, so
//! running it closes any other session on the same account — a
//! terminal, the desktop app, a script. Point it at a dedicated key.
//!
//!   THETADATA_API_KEY=… cargo run --release --example check_limits

use std::collections::BTreeMap;
use std::time::Duration;

use tdds_core::spec::{MAX_RANGE_DAYS, REJECTS_EXPIRATION_WILDCARD, REJECTS_TICK_INTERVAL};
use tdds_core::{Client, EndpointSpec};

const TIMEOUT_MS: u64 = 10_000;
/// A settled, ordinary trading day well inside every plan's history.
const DAY: &str = "20241202";

#[derive(Debug, PartialEq)]
enum Reply {
    /// Served, or accepted and began streaming.
    Accepted,
    /// Refused, with the server's reason.
    Refused(String),
}

#[tokio::main]
async fn main() {
    let client = match Client::connect(None).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cannot sign in: {e}");
            std::process::exit(2);
        }
    };
    let mut drift: Vec<String> = Vec::new();

    // ── 1. window cap ───────────────────────────────────────────────
    // `chunk_window` keeps every request at most MAX_RANGE_DAYS apart.
    // That width must be accepted, and one day more must be refused —
    // if the server now takes wider windows, the app is splitting for
    // nothing; if narrower, it is sending requests that fail.
    let start = chrono::NaiveDate::from_ymd_opt(2023, 1, 3).unwrap();
    let at_limit = start + chrono::Duration::days(MAX_RANGE_DAYS);
    let past_limit = at_limit + chrono::Duration::days(1);
    for (end, should_accept) in [(at_limit, true), (past_limit, false)] {
        let r = call(
            &client,
            "stock_history_eod",
            &[
                ("symbol", "SPY"),
                ("start_date", &start.format("%Y%m%d").to_string()),
                ("end_date", &end.format("%Y%m%d").to_string()),
            ],
        )
        .await;
        let accepted = r == Reply::Accepted;
        let span = (end - start).num_days();
        println!("window {span:>3} days      -> {r:?}");
        if accepted != should_accept {
            drift.push(format!(
                "window cap: a {span}-day range was {} (MAX_RANGE_DAYS = {MAX_RANGE_DAYS})",
                if accepted { "accepted" } else { "refused" }
            ));
        }
    }

    // ── 2. expiration wildcard ──────────────────────────────────────
    // Every option endpoint that takes an expiration is checked, not
    // just the ones in the list: an endpoint joining the list matters
    // as much as one leaving it.
    for meta in thetadatadx::ENDPOINTS {
        if !meta.name.starts_with("option_") || !meta.params.iter().any(|p| p.name == "expiration")
        {
            continue;
        }
        let args = option_args(meta, "*");
        let r = call(&client, meta.name, &borrow(&args)).await;
        let refuses =
            matches!(&r, Reply::Refused(m) if m.to_lowercase().contains("cannot specify"));
        let listed = REJECTS_EXPIRATION_WILDCARD.contains(&meta.name);
        println!(
            "expiration=* {:<46} -> {}",
            meta.name,
            if refuses { "refused" } else { "accepted" }
        );
        if refuses != listed {
            drift.push(format!(
                "{}: expiration=* is now {} but the app {}",
                meta.name,
                if refuses { "refused" } else { "accepted" },
                if listed {
                    "fans out over expirations for it"
                } else {
                    "sends the wildcard"
                }
            ));
        }
        pause().await;
    }

    // ── 3. tick interval ────────────────────────────────────────────
    for meta in thetadatadx::ENDPOINTS {
        let has = |n: &str| meta.params.iter().any(|p| p.name == n);
        if !has("interval") || !has("date") || meta.name.starts_with("option_") {
            continue;
        }
        let symbol = if meta.name.starts_with("index_") {
            "SPX"
        } else {
            "SPY"
        };
        let r = call(
            &client,
            meta.name,
            &[("symbol", symbol), ("date", DAY), ("interval", "tick")],
        )
        .await;
        let refuses = matches!(&r, Reply::Refused(m) if m.to_lowercase().contains("interval must be positive"));
        let listed = REJECTS_TICK_INTERVAL.contains(&meta.name);
        println!(
            "interval=tick {:<45} -> {}",
            meta.name,
            if refuses { "refused" } else { "accepted" }
        );
        if refuses != listed {
            drift.push(format!(
                "{}: interval=tick is now {} but the app {}",
                meta.name,
                if refuses { "refused" } else { "accepted" },
                if listed { "hides it" } else { "offers it" }
            ));
        }
        pause().await;
    }

    println!();
    if drift.is_empty() {
        println!("no drift: the server's constraints match what the app absorbs");
    } else {
        println!("DRIFT — update crates/tdds-core/src/spec.rs:");
        for d in &drift {
            println!("  - {d}");
        }
        std::process::exit(1);
    }
}

fn option_args(meta: &thetadatadx::EndpointMeta, expiration: &str) -> Vec<(String, String)> {
    let mut a = vec![
        ("symbol".into(), "AAPL".into()),
        ("expiration".into(), expiration.into()),
    ];
    for p in meta.params {
        let v = match p.name {
            "right" => "both",
            "strike" => "*",
            "request_type" => "trade",
            "date" | "start_date" | "end_date" => DAY,
            "interval" => "1h",
            "time_of_day" => "10:00:00.000",
            _ => continue,
        };
        a.push((p.name.to_string(), v.to_string()));
    }
    a
}

fn borrow(args: &[(String, String)]) -> Vec<(&str, &str)> {
    args.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
}

async fn pause() {
    tokio::time::sleep(Duration::from_millis(200)).await;
}

async fn call(client: &Client, endpoint: &str, args: &[(&str, &str)]) -> Reply {
    let mut spec = EndpointSpec::new(endpoint);
    spec.timeout_ms = TIMEOUT_MS;
    spec.args = args
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<BTreeMap<_, _>>();
    match tdds_core::dispatch_raw(client, &spec).await {
        Ok(_) => Reply::Accepted,
        Err(e) => {
            let msg = e.to_string();
            let lower = msg.to_lowercase();
            // No rows, or a stream we cut off, is the server saying yes.
            if lower.contains("no data found") || lower.contains("timeout") {
                Reply::Accepted
            } else {
                Reply::Refused(msg)
            }
        }
    }
}

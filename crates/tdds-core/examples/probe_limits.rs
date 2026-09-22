//! Probe every registered endpoint for server-side request limits.
//!
//! The app's job is to keep transport constraints away from the user, and
//! it can only do that for the ones it knows about. The 365-day window
//! cap on the range endpoints was found the way users find things — as a
//! failed download. This asks the server directly instead.
//!
//! Strategy: build the smallest valid argument set for each endpoint,
//! then deliberately overshoot one axis at a time. A cap answers
//! instantly with `InvalidArgument` and a message that usually states
//! the limit. No cap means the server starts streaming, so every call
//! runs under a short timeout and a timeout is recorded as "no cap on
//! this axis" — the point is to read the rejections, never to complete a
//! download.
//!
//! Read-only, and deliberately slow: one request at a time.
//!
//! Run where credentials live:
//!   cargo run --example probe_limits --features __internal
//!   (reads THETADATA_API_KEY, else ./creds.txt)

use std::collections::BTreeMap;
use std::time::Duration;

use tdds_core::{Client, EndpointSpec};

/// Short enough that a streaming response is cut off long before it
/// costs anything, long enough that a rejection always arrives first.
const PROBE_TIMEOUT_MS: u64 = 6_000;

/// Deliberately past any plausible window cap.
const WIDE_START: &str = "20140102";
const WIDE_END: &str = "20240102";
/// A single ordinary trading day, for endpoints that take one.
const ONE_DAY: &str = "20240102";

#[derive(Debug)]
enum Outcome {
    /// The server accepted it (or started streaming and we cut it off).
    Accepted,
    /// The server rejected it. This is the interesting case.
    Rejected(String),
    /// We could not build a valid request; not the server's fault.
    Skipped(String),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::connect(Some(std::path::Path::new("creds.txt"))).await?;
    println!("connected\n");

    let mut rejections: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();

    for meta in thetadatadx::ENDPOINTS {
        for (axis, args) in probes(meta) {
            let outcome = run_probe(&client, meta.name, &args).await;
            match &outcome {
                Outcome::Rejected(msg) => {
                    println!("REJECT  {:<44} {:<14} {msg}", meta.name, axis);
                    rejections
                        .entry(meta.name.to_string())
                        .or_default()
                        .push((axis.to_string(), msg.clone()));
                }
                Outcome::Accepted => println!("ok      {:<44} {axis}", meta.name),
                Outcome::Skipped(why) => println!("skip    {:<44} {axis:<14} {why}", meta.name),
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    println!("\n================ LIMITS FOUND ================");
    if rejections.is_empty() {
        println!("none");
    }
    for (endpoint, hits) in &rejections {
        for (axis, msg) in hits {
            println!("{endpoint}\n  axis: {axis}\n  {msg}\n");
        }
    }
    Ok(())
}

/// One probe per axis this endpoint actually exposes.
fn probes(meta: &thetadatadx::EndpointMeta) -> Vec<(&'static str, BTreeMap<String, String>)> {
    let has = |n: &str| meta.params.iter().any(|p| p.name == n);
    let mut out = Vec::new();

    // Axis 1: how wide a window will it take?
    if has("start_date") && has("end_date") {
        let mut a = base_args(meta);
        a.insert("start_date".into(), WIDE_START.into());
        a.insert("end_date".into(), WIDE_END.into());
        out.push(("wide-window", a));
    }

    // Axis 2: how many symbols at once? Only where the parameter is the
    // plural `Symbols` form.
    if meta
        .params
        .iter()
        .any(|p| p.name == "symbol" && format!("{:?}", p.param_type) == "Symbols")
    {
        let mut a = base_args(meta);
        let many: Vec<String> = (0..600).map(|i| format!("SYM{i}")).collect();
        a.insert("symbol".into(), many.join(","));
        out.push(("many-symbols", a));
    }

    // Axis 3: the finest granularity over a whole session, which is the
    // largest single-day response the endpoint can be asked for.
    if has("interval") && has("date") {
        let mut a = base_args(meta);
        a.insert("interval".into(), "tick".into());
        out.push(("tick-interval", a));
    }

    // Every endpoint gets at least a minimal call, so an endpoint with
    // no axis above is still exercised once.
    if out.is_empty() {
        out.push(("baseline", base_args(meta)));
    }
    out
}

/// The smallest argument set that satisfies what the endpoint declares.
fn base_args(meta: &thetadatadx::EndpointMeta) -> BTreeMap<String, String> {
    let mut a = BTreeMap::new();
    for p in meta.params {
        let v = match p.name {
            "symbol" => "AAPL",
            "expiration" => "*",
            "strike" => "*",
            "right" => "both",
            "date" => ONE_DAY,
            "start_date" => ONE_DAY,
            "end_date" => ONE_DAY,
            "request_type" => "trade",
            "interval" => "1h",
            "year" => "2024",
            "time_of_day" => "10:00:00.000",
            "annual_dividend" => "0",
            "rate_type" => "SOFR",
            "rate_value" => "0",
            "stock_price" => "0",
            // Anything else is optional; leaving it out is the point.
            _ => continue,
        };
        // Only fill an optional parameter when it is part of the probe.
        if p.required || matches!(p.name, "start_date" | "end_date" | "date" | "interval") {
            a.insert(p.name.to_string(), v.to_string());
        }
    }
    a
}

async fn run_probe(client: &Client, endpoint: &str, args: &BTreeMap<String, String>) -> Outcome {
    let mut spec = EndpointSpec::new(endpoint);
    spec.timeout_ms = PROBE_TIMEOUT_MS;
    for (k, v) in args {
        spec.args.insert(k.clone(), v.clone());
    }
    match tdds_core::dispatch_raw(client, &spec).await {
        Ok(_) => Outcome::Accepted,
        Err(e) => {
            let msg = e.to_string();
            let lower = msg.to_lowercase();
            // A cut-off stream is us, not the server.
            if lower.contains("timeout") || lower.contains("deadline") {
                Outcome::Accepted
            } else if lower.contains("missing required arg") || lower.contains("arg ") {
                Outcome::Skipped(msg)
            } else {
                Outcome::Rejected(msg)
            }
        }
    }
}

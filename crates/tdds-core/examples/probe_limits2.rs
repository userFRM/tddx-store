//! Second pass: isolate the constraints the first probe surfaced.
//!
//! Pass 1 overshot one axis at a time with placeholder arguments, which
//! is enough to find a cap but not to tell a cap from a bad argument.
//! This pass uses real values — real tickers, a real expiration, a real
//! index — so a rejection means the server refuses the *shape* of the
//! request rather than its contents.
//!
//! Four questions:
//!   1. How many symbols will a snapshot endpoint take at once?
//!   2. Which endpoints refuse `expiration=*`, and do they work with a
//!      concrete expiration?
//!   3. Which intervals does each interval-taking endpoint accept?
//!   4. Where is the earliest date a Free rates plan may ask for?

use std::collections::BTreeMap;
use std::time::Duration;

use tdds_core::{Client, EndpointSpec};

const T: u64 = 8_000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let c = Client::connect(Some(std::path::Path::new("creds.txt"))).await?;
    println!("connected\n");

    // Real symbols, straight from the server.
    let symbols = list(&c, "stock_list_symbols", &[]).await;
    println!("stock universe: {} symbols\n", symbols.len());

    // ── 1. symbol-count cap ──────────────────────────────────────
    println!("=== symbol count (stock_snapshot_quote) ===");
    for n in [1usize, 10, 50, 100, 250, 500, 1000, 2000] {
        if n > symbols.len() {
            break;
        }
        let joined = symbols[..n].join(",");
        let r = call(&c, "stock_snapshot_quote", &[("symbol", &joined)]).await;
        println!("  {n:>5} symbols -> {r}");
        pause().await;
    }

    // ── 2. expiration wildcard ───────────────────────────────────
    let exps = list(&c, "option_list_expirations", &[("symbol", "AAPL")]).await;
    let real_exp = exps
        .iter()
        .rev()
        .find(|e| e.as_str() < "20250101")
        .cloned()
        .or_else(|| exps.last().cloned())
        .unwrap_or_default();
    println!("\n=== expiration wildcard (AAPL, concrete exp = {real_exp}) ===");
    let exp_sensitive = [
        "option_list_dates",
        "option_list_strikes",
        "option_snapshot_trade",
        "option_history_ohlc",
        "option_history_greeks_all",
        "option_history_greeks_first_order",
        "option_history_greeks_second_order",
        "option_history_greeks_third_order",
        "option_history_greeks_implied_volatility",
    ];
    for ep in exp_sensitive {
        let meta = thetadatadx::find(ep).unwrap();
        let wants = |n: &str| meta.params.iter().any(|p| p.name == n);
        for (label, exp) in [("wildcard", "*"), ("concrete", real_exp.as_str())] {
            let mut args: Vec<(&str, &str)> = vec![("symbol", "AAPL"), ("expiration", exp)];
            if wants("right") {
                args.push(("right", "both"));
            }
            if wants("strike") {
                args.push(("strike", "*"));
            }
            if wants("request_type") {
                args.push(("request_type", "trade"));
            }
            if wants("date") {
                args.push(("date", "20241202"));
            }
            if wants("start_date") {
                args.push(("start_date", "20241202"));
            }
            if wants("end_date") {
                args.push(("end_date", "20241202"));
            }
            if wants("interval") {
                args.push(("interval", "1h"));
            }
            let r = call(&c, ep, &args).await;
            println!("  {ep:<46} {label:<9} -> {r}");
            pause().await;
        }
    }

    // ── 3. interval acceptance ───────────────────────────────────
    println!("\n=== interval acceptance ===");
    let interval_eps = [
        ("stock_history_ohlc", "AAPL", None),
        ("stock_history_quote", "AAPL", None),
        ("option_history_ohlc", "AAPL", Some(real_exp.as_str())),
        ("option_history_quote", "AAPL", Some(real_exp.as_str())),
        ("index_history_ohlc", "SPX", None),
        ("index_history_price", "SPX", None),
    ];
    for (ep, sym, exp) in interval_eps {
        let meta = match thetadatadx::find(ep) {
            Some(m) => m,
            None => continue,
        };
        let wants = |n: &str| meta.params.iter().any(|p| p.name == n);
        let mut accepted = Vec::new();
        let mut refused = Vec::new();
        for iv in tdds_core::INTERVALS {
            let mut args: Vec<(&str, &str)> = vec![("symbol", sym), ("interval", iv.id)];
            if let Some(e) = exp {
                args.push(("expiration", e));
            }
            if wants("right") {
                args.push(("right", "both"));
            }
            if wants("strike") {
                args.push(("strike", "*"));
            }
            if wants("date") {
                args.push(("date", "20241202"));
            }
            if wants("start_date") {
                args.push(("start_date", "20241202"));
            }
            if wants("end_date") {
                args.push(("end_date", "20241202"));
            }
            match call(&c, ep, &args).await {
                s if s.starts_with("ok") || s.starts_with("no-data") => accepted.push(iv.id),
                s => refused.push((iv.id, s)),
            }
            pause().await;
        }
        println!("  {ep}");
        println!("    accepts: {}", accepted.join(" "));
        for (iv, why) in &refused {
            println!("    refuses {iv:<6} {why}");
        }
    }

    // ── 4. rates date floor on this plan ─────────────────────────
    println!("\n=== interest_rate_history_eod date floor ===");
    for (s, e) in [
        ("20200101", "20200201"),
        ("20230101", "20230201"),
        ("20231201", "20240115"),
        ("20240101", "20240201"),
        ("20250101", "20250201"),
    ] {
        let r = call(
            &c,
            "interest_rate_history_eod",
            &[("rate_type", "SOFR"), ("start_date", s), ("end_date", e)],
        )
        .await;
        println!("  {s}..{e} -> {r}");
        pause().await;
    }
    Ok(())
}

async fn pause() {
    tokio::time::sleep(Duration::from_millis(200)).await;
}

async fn call(c: &Client, endpoint: &str, args: &[(&str, &str)]) -> String {
    let mut spec = EndpointSpec::new(endpoint);
    spec.timeout_ms = T;
    let mut m = BTreeMap::new();
    for (k, v) in args {
        m.insert(k.to_string(), v.to_string());
    }
    spec.args = m;
    match tdds_core::dispatch_raw(c, &spec).await {
        Ok(_) => "ok".to_string(),
        Err(e) => {
            let s = e.to_string();
            if s.to_lowercase().contains("no data found") {
                "no-data".to_string()
            } else if let Some(i) = s.find("message: ") {
                let tail = &s[i + 9..];
                let end = tail.find(", retry_after").unwrap_or(tail.len());
                format!("REJECT {}", tail[..end].trim_matches('"'))
            } else {
                format!("ERR {s}")
            }
        }
    }
}

async fn list(c: &Client, endpoint: &str, args: &[(&str, &str)]) -> Vec<String> {
    let mut spec = EndpointSpec::new(endpoint);
    spec.timeout_ms = 30_000;
    for (k, v) in args {
        spec.args.insert(k.to_string(), v.to_string());
    }
    match tdds_core::dispatch_raw(c, &spec).await {
        Ok(thetadatadx::EndpointOutput::StringList(v)) => v,
        _ => vec![],
    }
}

//! Subscription-tier gating.
//!
//! ThetaData partitions data access into per-asset-class tiers. We
//! surface the user's tiers, the per-endpoint requirement, and the
//! upgrade URL so the UI can grey out gated cards before queueing
//! requests the server would reject. See `tdds_core::tier`.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use tdds_core::tier::AssetClass;
use tdds_core::{all_endpoints, tier_evaluate, Tier, TierVerdict, UserTiers, UPGRADE_URL};

use crate::state::AppState;

/// Per-class tier slice. Single source of truth for the FE — name,
/// label, tier, worker count, and "at max" flag are all derived
/// server-side from `tdds_core::tier`. The FE iterates `classes` and
/// never recomputes any of these, so a change to `Tier::workers()` or
/// the asset-class set propagates automatically.
#[derive(Serialize)]
pub struct ClassTier {
    /// Wire name: `stock` | `option` | `index` | `rate`.
    pub class: String,
    /// Display label: `Stocks` | `Options` | `Indices` | `Rates`.
    pub label: &'static str,
    /// Tier name, already normalized: an `Unknown` class on a
    /// connected user collapses to `Free` (ThetaData grants free-tier
    /// access to every account by default; `Unknown` only ever means
    /// "the SDK didn't surface a purchased byte"). Pre-connect the
    /// raw `Unknown` is preserved so the UI can render "—".
    pub tier: String,
    pub workers: u32,
    /// True when this class is at the highest tier (Pro). UI uses
    /// this to hide the per-class upgrade affordance.
    pub at_max: bool,
}

#[derive(Serialize)]
pub struct TierStatus {
    pub stock: String,
    pub options: String,
    pub indices: String,
    pub interest_rate: String,
    /// Iterable per-class view. Replaces every FE-side hardcoded
    /// `[Stocks, Options, Indices, Rates]` list and every `2^tier`
    /// duplicate of `Tier::workers()`.
    pub classes: Vec<ClassTier>,
    /// Sum of `classes[*].workers` — the total parallel-download
    /// budget the user's subscription unlocks across every asset class.
    pub total_workers: u32,
    pub upgrade_url: &'static str,
    /// `false` until the user is connected — UI should treat this the
    /// same as all tiers being `Unknown` (most-restrictive view).
    pub connected: bool,
}

fn tier_label(t: Tier) -> String {
    t.as_str().to_string()
}

/// Apply the "Unknown == Free" policy when the user is connected.
/// Pre-connect we leave `Unknown` alone so the UI can show a neutral
/// placeholder; post-connect a missing byte means the SDK didn't
/// surface a purchase, which downgrades cleanly to the free baseline
/// every account always has access to.
fn normalize(t: Tier, connected: bool) -> Tier {
    if connected && matches!(t, Tier::Unknown) {
        Tier::Free
    } else {
        t
    }
}

fn class_row(class: AssetClass, label: &'static str, raw: Tier, connected: bool) -> ClassTier {
    let t = normalize(raw, connected);
    ClassTier {
        class: class.as_str().to_string(),
        label,
        tier: tier_label(t),
        workers: t.workers() as u32,
        at_max: matches!(t, Tier::Pro),
    }
}

#[tauri::command]
pub async fn tier_status(state: State<'_, Arc<AppState>>) -> Result<TierStatus, String> {
    let client_opt = state.client.read().await.as_ref().cloned();
    let (tiers, connected) = match client_opt {
        Some(c) => (c.user_tiers(), true),
        None => (UserTiers::default(), false),
    };
    let classes = vec![
        class_row(AssetClass::Stock, "Stocks", tiers.stock, connected),
        class_row(AssetClass::Option, "Options", tiers.options, connected),
        class_row(AssetClass::Index, "Indices", tiers.indices, connected),
        class_row(AssetClass::Rate, "Rates", tiers.interest_rate, connected),
    ];
    let total_workers = classes.iter().map(|c| c.workers).sum();
    Ok(TierStatus {
        stock: classes[0].tier.clone(),
        options: classes[1].tier.clone(),
        indices: classes[2].tier.clone(),
        interest_rate: classes[3].tier.clone(),
        classes,
        total_workers,
        upgrade_url: UPGRADE_URL,
        connected,
    })
}

/// Per-endpoint tier verdict for all 61 registered endpoints. Frontend
/// uses this to render an "Available on your tier" badge per dataset
/// card and to disable the queue button when not allowed.
#[tauri::command]
pub async fn tier_endpoints(state: State<'_, Arc<AppState>>) -> Result<Vec<TierVerdict>, String> {
    let client_opt = state.client.read().await.as_ref().cloned();
    let user = client_opt.map(|c| c.user_tiers()).unwrap_or_default();
    Ok(all_endpoints()
        .iter()
        .map(|info| tier_evaluate(info, &user))
        .collect())
}

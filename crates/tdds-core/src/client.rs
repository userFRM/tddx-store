//! Thin wrapper around thetadatadx::ThetaDataDxClient with a creds path that
//! falls through env (`DATADOCK_CREDS`), explicit override, then default.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use thetadatadx::{Credentials, DirectConfig, ThetaDataDxClient};

use crate::tier::{Tier, UserTiers};

#[derive(Clone)]
pub struct Client {
    inner: Arc<ThetaDataDxClient>,
}

impl Client {
    /// Connect with credentials from `creds_path` (or `DATADOCK_CREDS` env,
    /// or `./creds.txt`). rustls crypto provider is installed on first call.
    pub async fn connect(creds_path: Option<&Path>) -> crate::Result<Self> {
        Self::install_crypto();
        let path: PathBuf = match creds_path {
            Some(p) => p.to_path_buf(),
            None => match std::env::var("DATADOCK_CREDS") {
                Ok(v) => PathBuf::from(v),
                Err(_) => PathBuf::from("creds.txt"),
            },
        };
        let creds = Credentials::from_file(&path)?;
        Self::connect_with(creds).await
    }

    /// Connect with explicit email + password (no file involved).
    /// Used by the GUI login form.
    pub async fn connect_with_credentials(email: &str, password: &str) -> crate::Result<Self> {
        Self::install_crypto();
        let creds = Credentials::new(email, password);
        Self::connect_with(creds).await
    }

    async fn connect_with(creds: Credentials) -> crate::Result<Self> {
        let cfg = DirectConfig::production();
        let inner = ThetaDataDxClient::connect(&creds, cfg).await?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    fn install_crypto() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
    }

    pub fn raw(&self) -> &ThetaDataDxClient {
        &self.inner
    }

    /// User's per-asset-class subscription tiers, decoded from the auth
    /// response captured at connect time. ThetaData's Nexus carries
    /// four bytes (`stock_subscription`, `options_subscription`,
    /// `indices_subscription`, `interest_rate_subscription`); the
    /// upstream `SubscriptionInfo` struct in `thetadatadx` v10 only
    /// surfaces the first two, so `indices` / `interest_rate` come
    /// back `Tier::Unknown` until the SDK exposes accessors for them
    /// (the values are present on the wire — we just can't reach
    /// them through the public API yet).
    pub fn user_tiers(&self) -> UserTiers {
        let info = self.inner.subscription_info();
        UserTiers {
            stock: Tier::from_label(&info.stock),
            options: Tier::from_label(&info.options),
            indices: Tier::Unknown,
            interest_rate: Tier::Unknown,
        }
    }

    /// All trading dates the server has TRADE data for, intersected with
    /// `[start, end]`. Used to drive per-day fan-out.
    pub async fn trading_days(
        &self,
        symbol: &str,
        start: chrono::NaiveDate,
        end: chrono::NaiveDate,
    ) -> crate::Result<Vec<chrono::NaiveDate>> {
        let raw = self.inner.stock_list_dates("TRADE", symbol).await?;
        Ok(raw
            .iter()
            .filter_map(|s| chrono::NaiveDate::parse_from_str(&s.replace('-', ""), "%Y%m%d").ok())
            .filter(|d| *d >= start && *d <= end)
            .collect())
    }
}

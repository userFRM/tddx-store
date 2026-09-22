//! Thin wrapper around `thetadatadx::Client`.
//!
//! ThetaData accepts either an API key or an email and password, and so
//! does this app. A key is the better credential for a downloader: it is
//! revocable from the account portal without changing the password, and
//! it is the only form the `THETADATA_API_KEY` environment variable
//! carries.
//!
//! Sourcing order when no credential is supplied inline:
//! `THETADATA_API_KEY`, then the two-line `creds.txt` at `creds_path`
//! (or `DATADOCK_CREDS`, or `./creds.txt`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use thetadatadx::{Credentials, DirectConfig, MarketDataClient};

use crate::tier::{Tier, UserTiers};

#[derive(Clone)]
pub struct Client {
    inner: Arc<thetadatadx::Client>,
}

impl Client {
    /// Connect with whatever credential the environment or `creds_path`
    /// supplies. `THETADATA_API_KEY` wins when it is set and non-empty;
    /// otherwise the two-line email + password file is read.
    ///
    /// The rustls crypto provider is installed on first call.
    pub async fn connect(creds_path: Option<&Path>) -> crate::Result<Self> {
        Self::install_crypto();
        let path: PathBuf = match creds_path {
            Some(p) => p.to_path_buf(),
            None => match std::env::var("DATADOCK_CREDS") {
                Ok(v) => PathBuf::from(v),
                Err(_) => PathBuf::from("creds.txt"),
            },
        };
        let creds = Credentials::from_env_or_file(&path)?;
        Self::connect_with(creds).await
    }

    /// Connect with explicit email + password (no file involved).
    pub async fn connect_with_credentials(email: &str, password: &str) -> crate::Result<Self> {
        Self::install_crypto();
        let creds = Credentials::new(email, password);
        Self::connect_with(creds).await
    }

    /// Connect with an API key issued from the ThetaData account portal.
    pub async fn connect_with_api_key(api_key: &str) -> crate::Result<Self> {
        Self::install_crypto();
        let creds = Credentials::api_key(api_key);
        Self::connect_with(creds).await
    }

    async fn connect_with(creds: Credentials) -> crate::Result<Self> {
        let cfg = DirectConfig::production();
        let inner = thetadatadx::Client::connect(&creds, cfg).await?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    fn install_crypto() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
    }

    pub fn raw(&self) -> &thetadatadx::Client {
        &self.inner
    }

    /// The market-data query surface every endpoint dispatch runs through.
    pub fn market_data(&self) -> &MarketDataClient {
        self.inner.market_data()
    }

    /// User's per-asset-class subscription tiers, decoded from the auth
    /// response captured at connect time. ThetaData's Nexus carries four
    /// fields (`stock_subscription`, `options_subscription`,
    /// `indices_subscription`, `interest_rate_subscription`) and
    /// `SubscriptionInfo` surfaces all four; a field the auth response
    /// omitted comes back as the literal `"Unknown"`, which
    /// `Tier::from_label` maps to `Tier::Unknown`.
    pub fn user_tiers(&self) -> UserTiers {
        let info = self.inner.subscription_info();
        UserTiers {
            stock: Tier::from_label(&info.stock),
            options: Tier::from_label(&info.options),
            indices: Tier::from_label(&info.indices),
            interest_rate: Tier::from_label(&info.interest_rate),
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
        let raw = self
            .inner
            .market_data()
            .stock_list_dates("TRADE", symbol)
            .await?;
        Ok(raw
            .iter()
            .filter_map(|s| chrono::NaiveDate::parse_from_str(&s.replace('-', ""), "%Y%m%d").ok())
            .filter(|d| *d >= start && *d <= end)
            .collect())
    }
}

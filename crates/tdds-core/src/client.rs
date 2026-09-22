//! Thin wrapper around `thetadatadx::MarketDataClient`.
//!
//! Deliberately the market-data client, not the unified one. This app
//! only ever pulls historical data and flat files, and the unified
//! `Client` additionally owns a streaming surface whose consumer
//! defaults to a spin wait — "~100% of one core" by the SDK's own
//! description. Holding a core to run no stream is indefensible in a
//! desktop tool, and configuring that away treats the symptom; not
//! constructing the surface removes it.
//!
//! It also keeps the app out of the way. Streaming connections are
//! metered per account, so a store that quietly held one would compete
//! with whatever the user runs alongside it. Nothing here touches that
//! budget.
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
    inner: Arc<MarketDataClient>,
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
        let inner = MarketDataClient::connect(&creds, DirectConfig::production()).await?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    fn install_crypto() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
    }

    /// The query surface every endpoint dispatch and flat-file pull
    /// runs through.
    pub fn raw(&self) -> &MarketDataClient {
        &self.inner
    }

    /// User's per-asset-class subscription tiers, captured from the auth
    /// response at connect time. The client exposes these as a typed
    /// enum per class, so nothing here parses a label; a class the
    /// response omitted arrives as `None` and maps to `Tier::Unknown`.
    pub fn user_tiers(&self) -> UserTiers {
        UserTiers {
            stock: Tier::from_sdk(self.inner.stock_tier()),
            options: Tier::from_sdk(self.inner.options_tier()),
            indices: Tier::from_sdk(self.inner.indices_tier()),
            interest_rate: Tier::from_sdk(self.inner.interest_rate_tier()),
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

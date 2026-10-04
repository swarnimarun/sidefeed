pub mod ai;
pub mod api;
// --- lane-authsec: scoped keys + rate limits (Tasks 1 + 8) ---
pub mod auth;
pub mod ratelimit;
// --- end lane-authsec Tasks 1+8 ---
pub mod channels;
pub mod config;
pub mod enrich;
pub mod error;
pub mod federation;
pub mod ingest;
pub mod model;
pub mod store;

use std::{sync::Arc, time::Duration};
use reqwest::Client;
use tokio::sync::broadcast;
use crate::{config::Config, error::Result, model::Item, store::Store};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub store: Store,
    pub http: Client,
    pub events: broadcast::Sender<Item>,
    /// Bounded cache of derived artifacts. Deliberately the only thing about
    /// enrichment that lives in memory; everything else stays on disk.
    pub atoms: Arc<enrich::AtomCache>,
    /// The most recent per-category digest, rebuilt at most once per TTL.
    pub updates: Arc<enrich::UpdatesCache>,
}

impl AppState {
    pub async fn new(config: Config) -> Result<Self> {
        let store = Store::connect(&config.database_url).await?;
        let http = Client::builder()
            .user_agent(format!("sidefeed/{}", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(5))
            .timeout(config.fetch_timeout)
            // Redirects are followed by the ingestion layer so every hop is
            // checked against the private-network denylist.
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let (events, _) = broadcast::channel(256);
        Ok(Self { config: Arc::new(config), store, http, events, atoms: Arc::new(enrich::AtomCache::default()), updates: Arc::new(enrich::UpdatesCache::default()) })
    }
}

pub mod ai;
pub mod api;
pub mod config;
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
}

impl AppState {
    pub async fn new(config: Config) -> Result<Self> {
        let store = Store::connect(&config.database_url).await?;
        let http = Client::builder()
            .user_agent(format!("sidefeed/{}", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(5))
            .timeout(config.fetch_timeout)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;
        let (events, _) = broadcast::channel(256);
        Ok(Self { config: Arc::new(config), store, http, events })
    }
}


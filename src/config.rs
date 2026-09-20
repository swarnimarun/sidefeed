use std::{env, net::SocketAddr, str::FromStr, time::Duration};
use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub database_url: String,
    pub public_url: String,
    pub fetch_interval: Duration,
    pub fetch_timeout: Duration,
    pub max_response_bytes: usize,
    pub peer_max_items: u32,
    pub admin_token: Option<String>,
    pub embedding_url: Option<String>,
    pub embedding_token: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            listen: parse("SIDEFEED_LISTEN", "0.0.0.0:8080")?,
            database_url: env::var("SIDEFEED_DATABASE_URL").unwrap_or_else(|_| "sqlite://sidefeed.db?mode=rwc".into()),
            public_url: env::var("SIDEFEED_PUBLIC_URL").unwrap_or_else(|_| "http://localhost:8080".into()).trim_end_matches('/').to_owned(),
            fetch_interval: Duration::from_secs(parse("SIDEFEED_FETCH_INTERVAL_SECONDS", "900")?),
            fetch_timeout: Duration::from_secs(parse("SIDEFEED_FETCH_TIMEOUT_SECONDS", "20")?),
            max_response_bytes: parse("SIDEFEED_MAX_RESPONSE_BYTES", "5242880")?,
            peer_max_items: parse("SIDEFEED_PEER_MAX_ITEMS", "500")?,
            admin_token: env::var("SIDEFEED_ADMIN_TOKEN").ok().filter(|v| !v.is_empty()),
            embedding_url: env::var("SIDEFEED_EMBEDDING_URL").ok().filter(|v| !v.is_empty()),
            embedding_token: env::var("SIDEFEED_EMBEDDING_TOKEN").ok().filter(|v| !v.is_empty()),
        })
    }
}

fn parse<T: FromStr>(name: &str, default: &str) -> Result<T> {
    env::var(name).unwrap_or_else(|_| default.to_owned()).parse()
        .map_err(|_| Error::Config(format!("invalid {name}")))
}


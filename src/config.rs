use std::{env, net::SocketAddr, path::PathBuf, str::FromStr, time::Duration};
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
    pub retention_days: u32,
    pub admin_token: Option<String>,
    pub embedding_url: Option<String>,
    pub embedding_token: Option<String>,
    pub embedding_provider: String,
    /// Derived-artifact settings: tags and generated summaries.
    pub enrich: EnrichConfig,
    /// Optional directory that overrides the embedded UI assets at runtime.
    pub web_dir: Option<PathBuf>,
}

/// Local text enrichment settings. Artifacts are stored on disk in SQLite;
/// only `cache_entries` of them are ever held in memory.
#[derive(Debug, Clone)]
pub struct EnrichConfig {
    /// disabled | heuristic | openai
    pub provider: String,
    /// Chat-completions endpoint for the openai provider, for example a local
    /// Ollama at http://127.0.0.1:11434/v1/chat/completions.
    pub url: Option<String>,
    pub token: Option<String>,
    pub model: Option<String>,
    /// Input text is truncated to this many characters before it reaches a
    /// model, which bounds both cost and memory per item.
    pub max_chars: usize,
    /// Items processed per worker pass.
    pub batch: u32,
    pub interval: Duration,
    /// Upper bound on the in-memory artifact cache.
    pub cache_entries: usize,
}

impl Default for EnrichConfig {
    fn default() -> Self {
        Self {
            provider: "disabled".into(), url: None, token: None, model: None,
            max_chars: 8000, batch: 4, interval: Duration::from_secs(60), cache_entries: 128,
        }
    }
}

impl EnrichConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            provider: env::var("SIDEFEED_ENRICH_PROVIDER").unwrap_or_else(|_| "disabled".into()),
            url: env::var("SIDEFEED_ENRICH_URL").ok().filter(|value| !value.is_empty()),
            token: env::var("SIDEFEED_ENRICH_TOKEN").ok().filter(|value| !value.is_empty()),
            model: env::var("SIDEFEED_ENRICH_MODEL").ok().filter(|value| !value.is_empty()),
            max_chars: parse("SIDEFEED_ENRICH_MAX_CHARS", "8000")?,
            batch: parse("SIDEFEED_ENRICH_BATCH", "4")?,
            interval: Duration::from_secs(parse("SIDEFEED_ENRICH_INTERVAL_SECONDS", "60")?),
            cache_entries: parse("SIDEFEED_ENRICH_CACHE_ENTRIES", "128")?,
        })
    }

    pub fn enabled(&self) -> bool { self.provider != "disabled" }
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
            retention_days: parse("SIDEFEED_RETENTION_DAYS", "90")?,
            admin_token: env::var("SIDEFEED_ADMIN_TOKEN").ok().filter(|v| !v.is_empty()),
            embedding_url: env::var("SIDEFEED_EMBEDDING_URL").ok().filter(|v| !v.is_empty()),
            embedding_token: env::var("SIDEFEED_EMBEDDING_TOKEN").ok().filter(|v| !v.is_empty()),
            embedding_provider: env::var("SIDEFEED_EMBEDDING_PROVIDER").unwrap_or_else(|_| "disabled".into()),
            enrich: EnrichConfig::from_env()?,
            web_dir: env::var("SIDEFEED_WEB_DIR").ok().filter(|v| !v.is_empty()).map(PathBuf::from),
        })
    }
}

fn parse<T: FromStr>(name: &str, default: &str) -> Result<T> {
    env::var(name).unwrap_or_else(|_| default.to_owned()).parse()
        .map_err(|_| Error::Config(format!("invalid {name}")))
}

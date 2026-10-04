use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Source {
    pub id: String, pub url: String, pub kind: String, pub title: Option<String>,
    pub etag: Option<String>, pub last_modified: Option<String>, pub last_polled_at: Option<String>,
    pub next_poll_at: Option<String>, pub last_error: Option<String>, pub enabled: bool, pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Item {
    pub id: String, pub source_id: Option<String>, pub external_id: String, pub url: Option<String>,
    pub title: Option<String>, pub summary: Option<String>, pub content: Option<String>, pub author: Option<String>,
    pub published_at: String, pub fetched_at: String, pub tags_json: String, pub raw_json: Option<String>, pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Feed {
    pub id: String, pub slug: String, pub title: String, pub description: Option<String>,
    pub include_terms: Option<String>, pub exclude_terms: Option<String>, pub public: bool, pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Peer {
    pub id: String, pub base_url: String,
    #[serde(skip_serializing)] pub shared_secret: String,
    pub enabled: bool, pub last_sync_at: Option<String>, pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewItem {
    pub external_id: String, pub url: Option<String>, pub title: Option<String>, pub summary: Option<String>,
    pub content: Option<String>, pub author: Option<String>, pub published_at: String,
    #[serde(default)] pub tags: Vec<String>, #[serde(default)] pub raw: Option<serde_json::Value>,
    #[serde(default="public_visibility")] pub visibility: String,
}
fn public_visibility() -> String { "public".into() }

#[derive(Debug, Serialize)]
pub struct Page<T> { pub items: Vec<T>, pub next_cursor: Option<String> }


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
    pub published_at: String, pub date_source: String, pub fetched_at: String, pub tags_json: String, pub raw_json: Option<String>, pub visibility: String,
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
    #[serde(default = "published_source")] pub date_source: String,
    #[serde(default)] pub tags: Vec<String>, #[serde(default)] pub raw: Option<serde_json::Value>,
    #[serde(default="public_visibility")] pub visibility: String,
}
fn public_visibility() -> String { "public".into() }
/// NewItem arrives from peers as well as the parser; a peer item without the
/// field predates date tracking, so it is treated as a real feed date.
fn published_source() -> String { "published".into() }

#[derive(Debug, Serialize)]
pub struct Page<T> { pub items: Vec<T>, pub next_cursor: Option<String> }

/// Whatever an enricher derived for one item. Small by construction: a couple of
/// tags and at most a few sentences.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Atoms {
    #[serde(default)] pub tags: Vec<String>,
    #[serde(default)] pub summary: Option<String>,
}

/// An item plus its derived artifacts. Flattened so every existing consumer of
/// an item keeps working; the reader reads `tags` and `ai_summary`.
#[derive(Debug, Clone, Serialize)]
pub struct EnrichedItem {
    #[serde(flatten)] pub item: Item,
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub ai_summary: Option<String>,
}

impl EnrichedItem {
    pub fn new(item: Item, atoms: Atoms) -> Self {
        Self { item, tags: atoms.tags, ai_summary: atoms.summary }
    }
}

/// An item with the feed it came from, for cross-feed listings where the caller
/// has no slug to look up.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ItemWithFeed {
    #[sqlx(flatten)]
    pub item: Item,
    pub feed_slug: String,
    pub feed_title: String,
}

/// A bookmarked item plus the feed it was saved from and when it was saved.
#[derive(Debug, Clone, FromRow)]
pub struct SavedItem {
    #[sqlx(flatten)]
    pub item: Item,
    pub feed_slug: String,
    pub feed_title: String,
    pub saved_at: String,
}


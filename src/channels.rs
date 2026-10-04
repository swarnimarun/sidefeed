//! Raw-channel ingest: signed webhook payloads (Task 2) and the generic
//! `raw-json` REST poller (Task 4).
//!
//! A webhook channel is a named ingress endpoint bound to one source.
//! Senders authenticate with either a `Bearer <secret>` token or an
//! `x-sidefeed-signature` HMAC header. Only the SHA-256 hash of the secret is
//! stored, so the HMAC key is that stored hash rendered as hex: the sender
//! knows the secret, hashes it the same way, and computes
//! `HMAC-SHA256(key = hex(sha256(secret)), body)`. The Bearer form is simpler
//! and preferred; the header form exists for senders that sign at the edge.

use axum::http::{HeaderMap, StatusCode};
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;
use crate::{
    error::{Error, Result},
    model::{NewItem, Source, WebhookChannel},
    AppState,
};

/// Channel secrets hash exactly like API tokens; one construction, one place.
/// Stored values are byte-identical to earlier `hash_secret` output.
pub use crate::auth::hash_token as hash_secret;

/// At most one webhook batch carries this many items. Above it the ingress
/// route answers 413 instead of partially ingesting.
pub const MAX_WEBHOOK_ITEMS: usize = 100;

/// Check the request against the channel credential. Accepts `Authorization:
/// Bearer <secret>` (constant-shaped compare on the hash) or
/// `x-sidefeed-signature: sha256=<hex hmac>` over the raw body keyed by the
/// stored secret hash.
pub fn verify_ingress(channel: &WebhookChannel, headers: &HeaderMap, body: &[u8]) -> Result<()> {
    if let Some(supplied) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        if hash_secret(supplied) == channel.secret_hash {
            return Ok(());
        }
        return Err(Error::Unauthorized);
    }
    if let Some(signature) = headers.get("x-sidefeed-signature").and_then(|v| v.to_str().ok()) {
        let hex_part = signature.strip_prefix("sha256=").unwrap_or(signature);
        let expected = hmac_hex(channel.secret_hash.as_bytes(), body);
        if constant_time_eq(&expected, &hex_part.to_lowercase()) {
            return Ok(());
        }
        return Err(Error::Unauthorized);
    }
    Err(Error::Unauthorized)
}

fn hmac_hex(key: &[u8], body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes().zip(right.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

/// Normalize a webhook body into canonical items. Accepts either
/// `{"items": [...]}` or a bare `[...]` array; every entry needs an `id`
/// (falling back to its `url`, then to a generated id so a sloppy sender
/// still ingests). More than [`MAX_WEBHOOK_ITEMS`] entries is a
/// 413 at the route, not a silent truncation.
pub fn normalize_webhook(body: &[u8]) -> std::result::Result<Vec<NewItem>, StatusCode> {
    let value: Value = serde_json::from_slice(body).map_err(|_| StatusCode::BAD_REQUEST)?;
    let list = match &value {
        Value::Array(items) => items.clone(),
        Value::Object(map) => map
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .ok_or(StatusCode::BAD_REQUEST)?,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    if list.len() > MAX_WEBHOOK_ITEMS {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    list.iter().map(map_entry).collect()
}

fn string_field(entry: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| entry.get(*key).and_then(Value::as_str).map(str::to_owned))
}

// ---------------------------------------------------------------- raw-json poller (Task 4)
// A generic REST poller for JSON APIs that are not feeds: the config points
// at the array (`items_pointer`, e.g. `/data/posts`) and names the fields.
// Unconfigured fields fall back through the usual feed-ish names so
// `{"items": [{"id", "url", "title"}]}` works with an empty config.

#[derive(Debug, Clone, Deserialize)]
pub struct RawJsonConfig {
    #[serde(default = "default_pointer")]
    pub items_pointer: String,
    #[serde(default = "default_id")]
    pub id_field: String,
    pub url_field: Option<String>,
    pub title_field: Option<String>,
    pub content_field: Option<String>,
    pub date_field: Option<String>,
}

impl Default for RawJsonConfig {
    fn default() -> Self {
        Self {
            items_pointer: default_pointer(),
            id_field: default_id(),
            url_field: None,
            title_field: None,
            content_field: None,
            date_field: None,
        }
    }
}

fn default_pointer() -> String {
    "/items".into()
}
fn default_id() -> String {
    "id".into()
}

/// A raw channel carries at most this many entries per poll; above it the
/// source errors instead of ingesting a truncated page as the whole feed.
const MAX_RAW_ITEMS: usize = 500;

/// Walk a `/a/b/0/c` pointer; object keys plus numeric array indices.
/// A dotted field name (`a.b`) walks one extra object level.
fn lookup<'a>(entry: &'a Value, field: &str) -> Option<&'a Value> {
    let mut current = entry;
    for part in field.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

fn text_at(entry: &Value, configured: Option<&str>, fallbacks: &[&str]) -> Option<String> {
    if let Some(field) = configured {
        if let Some(text) = lookup(entry, field).and_then(Value::as_str) {
            return Some(text.to_owned());
        }
    }
    fallbacks.iter().find_map(|key| entry.get(*key).and_then(Value::as_str).map(str::to_owned))
}

/// Select the item array a pointer addresses and map every entry.
/// Returns [`Error::Invalid`] when the pointer misses, hits a scalar, or
/// selects a non-array: a misconfiguration, never an empty page. Entries
/// that are not objects are skipped, so one malformed row does not fail the
/// poll; entries without any usable id get a generated one. At most
/// [`MAX_RAW_ITEMS`] entries are read.
pub fn select_items(doc: &Value, config: &RawJsonConfig) -> Result<Vec<NewItem>> {
    let mut current = doc;
    for part in config.items_pointer.split('/').filter(|p| !p.is_empty()) {
        current = match current {
            Value::Array(list) => part
                .parse::<usize>()
                .ok()
                .and_then(|i| list.get(i))
                .ok_or_else(|| Error::Invalid("items_pointer is out of range".into()))?,
            Value::Object(map) => map.get(part).ok_or_else(|| Error::Invalid("items_pointer missed".into()))?,
            _ => return Err(Error::Invalid("items_pointer hit a scalar".into())),
        };
    }
    let list = current.as_array().ok_or_else(|| Error::Invalid("items_pointer must select an array".into()))?;
    if list.len() > MAX_RAW_ITEMS {
        return Err(Error::Invalid("raw channel item limit is 500".into()));
    }
    Ok(list.iter().filter_map(|entry| entry.as_object().map(|_| map_raw_entry(entry, config))).collect())
}

fn map_raw_entry(entry: &Value, config: &RawJsonConfig) -> NewItem {
    let url = text_at(entry, config.url_field.as_deref(), &["url", "link"]);
    let external_id = text_at(entry, Some(&config.id_field), &["id", "external_id", "uid"])
        .or_else(|| url.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let (published_at, date_source) = match text_at(entry, config.date_field.as_deref(), &["published", "published_at", "date", "ts"]) {
        Some(raw) => match chrono::DateTime::parse_from_rfc3339(&raw) {
            Ok(stamp) => (stamp.with_timezone(&Utc).to_rfc3339(), "published".into()),
            Err(_) => (Utc::now().to_rfc3339(), "fetched".into()),
        },
        None => (Utc::now().to_rfc3339(), "fetched".into()),
    };
    let tags = entry
        .get("tags")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    NewItem {
        external_id,
        url,
        title: text_at(entry, config.title_field.as_deref(), &["title", "headline", "name"]),
        summary: text_at(entry, None, &["summary", "description", "excerpt"]),
        content: text_at(entry, config.content_field.as_deref(), &["content", "content_html", "content_text", "body"]),
        author: text_at(entry, None, &["author", "actor"]),
        published_at,
        date_source,
        tags,
        raw: Some(entry.clone()),
        visibility: "public".into(),
    }
}

/// Poll one `raw-json` source: conditional GET against the stored validators,
/// then [`select_items`] plus upsert. Mirrors the feed poll's fetch behaviour
/// (redirects re-checked, responses capped) through the shared helper.
pub async fn poll_raw_json(state: &AppState, source: &Source, config: &RawJsonConfig) -> Result<usize> {
    let url = url::Url::parse(&source.url).map_err(|e| Error::Invalid(format!("invalid source URL: {e}")))?;
    let fetched =
        crate::ingest::fetch_validated(state, url, source.etag.as_deref(), source.last_modified.as_deref()).await?;
    let next = (Utc::now() + chrono::Duration::from_std(state.config.fetch_interval).unwrap_or(chrono::Duration::minutes(15))).to_rfc3339();
    if fetched.not_modified {
        state.store.update_source_fetch(&source.id, None, None, None, None, &next).await?;
        return Ok(0);
    }
    let doc: Value =
        serde_json::from_slice(&fetched.bytes).map_err(|e| Error::Invalid(format!("invalid raw JSON: {e}")))?;
    let mut count = 0;
    for item in select_items(&doc, config)? {
        let stored = state.store.upsert_item(Some(&source.id), &item).await?;
        let _ = state.events.send(stored);
        count += 1;
    }
    state.store.update_source_fetch(&source.id, None, fetched.etag.as_deref(), fetched.modified.as_deref(), None, &next).await?;
    Ok(count)
}

// Webhook entry mapping lives below; the raw poller above shares its shape
// but reads configurable fields instead of fixed ones.
fn map_entry(entry: &Value) -> std::result::Result<NewItem, StatusCode> {
    let entry = entry.as_object().ok_or(StatusCode::BAD_REQUEST)?;
    let entry = Value::Object(entry.clone());
    let url = string_field(&entry, &["url", "link"]);
    let external_id = string_field(&entry, &["id", "external_id", "uid"])
        .or_else(|| url.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let (published_at, date_source) = match string_field(&entry, &["published", "published_at", "date", "ts"]) {
        Some(raw) => match chrono::DateTime::parse_from_rfc3339(&raw) {
            Ok(stamp) => (stamp.with_timezone(&Utc).to_rfc3339(), "published".into()),
            Err(_) => (Utc::now().to_rfc3339(), "fetched".into()),
        },
        None => (Utc::now().to_rfc3339(), "fetched".into()),
    };
    let tags = entry
        .get("tags")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    Ok(NewItem {
        external_id,
        url,
        title: string_field(&entry, &["title", "headline", "name"]),
        summary: string_field(&entry, &["summary", "description", "excerpt"]),
        content: string_field(&entry, &["content", "content_html", "content_text", "body"]),
        author: string_field(&entry, &["author", "actor"]),
        published_at,
        date_source,
        tags,
        raw: Some(entry),
        visibility: "public".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel() -> WebhookChannel {
        WebhookChannel {
            id: "test".into(),
            slug: "deploys".into(),
            secret_hash: hash_secret("s3cret"),
            source_id: None,
            created_at: "2026-10-04T00:00:00Z".into(),
        }
    }

    #[test]
    fn bearer_and_hmac_signatures_both_verify() {
        let body = br#"{"items":[]}"#;
        let channel = channel();
        let bearer: HeaderMap = [(axum::http::header::AUTHORIZATION, "Bearer s3cret".parse().unwrap())]
            .into_iter()
            .collect();
        assert!(verify_ingress(&channel, &bearer, body).is_ok());

        let signature = format!("sha256={}", hmac_hex(channel.secret_hash.as_bytes(), body));
        let signed: HeaderMap =
            [("x-sidefeed-signature".parse().unwrap(), signature.parse().unwrap())].into_iter().collect();
        assert!(verify_ingress(&channel, &signed, body).is_ok());

        let wrong: HeaderMap = [(axum::http::header::AUTHORIZATION, "Bearer nope".parse().unwrap())]
            .into_iter()
            .collect();
        assert!(verify_ingress(&channel, &wrong, body).is_err());
        assert!(verify_ingress(&channel, &HeaderMap::new(), body).is_err());
    }

    #[test]
    fn webhook_batches_cap_at_one_hundred_items() {
        let big = Value::Array((0..101).map(|i| serde_json::json!({"id": format!("item-{i}")})).collect());
        assert_eq!(normalize_webhook(&serde_json::to_vec(&big).unwrap()).unwrap_err(), StatusCode::PAYLOAD_TOO_LARGE);
        let ok = serde_json::json!({"items": [{"id": "d1", "title": "deploy"}]});
        let items = normalize_webhook(&serde_json::to_vec(&ok).unwrap()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].external_id, "d1");
    }
}

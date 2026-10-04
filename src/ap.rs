//! ActivityPub read-plus-follow (no general server).
//!
//! Sidefeed follows remote actors and polls their outboxes like any other
//! source. It is deliberately not a server: there are no multi-user inboxes,
//! no relays, and the inbox only understands `Accept{Follow}` (marking the
//! matching source config `following:true`) and `Create` (normalizing one
//! item). The node actor, WebFinger document, and inbox are inert 404s unless
//! `SIDEFEED_AP_ENABLED=1`, because serving them is only needed to receive
//! Follow Accepts.
//!
//! Signatures are ed25519 over the `hs2019`-style signing string, keyed by the
//! single node seed in `node_meta`. Remote actors that sign with RSA (the
//! Mastodon default) fail verification with a clear error: full RSA interop
//! is deferred, polling their public outboxes needs no signatures at all.

use axum::http::{HeaderMap, StatusCode};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use futures_util::StreamExt;
use reqwest::{header::{ETAG, IF_NONE_MATCH}, Client};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use url::Url;
use uuid::Uuid;
use crate::{
    error::{Error, Result},
    ingest::{dated, string_field, validate_public_url},
    model::{NewItem, Source},
    store::Store,
    AppState,
};

const ACTIVITY_ACCEPT: &str = "application/activity+json, application/ld+json; profile=\"https://www.w3.org/ns/activitystreams\", application/json;q=0.1";
/// Outbox collection pages followed per poll; each hop is re-checked against
/// the SSRF denylist and size cap, so a hostile chain cannot page forever.
const MAX_OUTBOX_PAGES: usize = 3;
/// At most this many items are kept from one outbox page; excess is dropped
/// with a warn (mirrors the webhook 100 / raw 500 precedent).
const MAX_OUTBOX_ITEMS_PER_PAGE: usize = 200;
/// At most this many items are ingested from one poll_actor pass across all
/// pages; excess is dropped with a warn.
const MAX_OUTBOX_ITEMS_TOTAL: usize = 500;
/// Inbox deliveries older or newer than this are rejected outright.
const INBOX_SKEW_SECONDS: i64 = 300;

// ---------------------------------------------------------------- outbox read

/// Normalize one outbox entry: a bare object or a `Create`/`Announce`
/// wrapper around one. Anything else (likes, bare follows, tombstones) is
/// ignored. Returns `None` for non-item objects.
pub fn items_from_outbox(page: &Value, base: &Url) -> Result<Vec<NewItem>> {
    let list = page
        .get("orderedItems")
        .or_else(|| page.get("items"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_else(|| vec![page.clone()]);
    let mut items = Vec::new();
    for activity in list {
        // Accept bare objects plus Create/Announce wrappers; ignore the rest.
        let kind = activity.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let object = match kind {
            "Create" | "Announce" => activity.get("object").filter(|v| v.is_object()).unwrap_or(&activity),
            _ => &activity,
        };
        if let Some(item) = normalize_object(object, &activity, base) {
            items.push(item);
        }
    }
    if items.len() > MAX_OUTBOX_ITEMS_PER_PAGE {
        tracing::warn!(kept = MAX_OUTBOX_ITEMS_PER_PAGE, total = items.len(), "outbox page exceeds per-page cap; dropping excess");
        items.truncate(MAX_OUTBOX_ITEMS_PER_PAGE);
    }
    Ok(items)
}

/// The object-to-item mapping shared by the generic ingest path and the AP
/// poller. Moved here from `ingest.rs` so the two never drift; `ingest.rs`
/// calls this rather than keeping its own copy.
pub(crate) fn normalize_object(object: &Value, activity: &Value, base: &Url) -> Option<NewItem> {
    let kind = object.get("type").and_then(Value::as_str).unwrap_or("");
    if !matches!(kind, "Note" | "Article" | "Page" | "Event") {
        return None;
    }
    let id = string_field(object, "id")
        .or_else(|| string_field(object, "url"))
        .unwrap_or_else(|| base.to_string());
    let actor = object
        .get("attributedTo")
        .and_then(|v| v.as_str())
        .or_else(|| activity.get("actor").and_then(Value::as_str))
        .map(str::to_owned);
    let tags = object
        .get("tag")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.get("name").and_then(Value::as_str).map(str::to_owned))
        .collect();
    let (published_at, date_source) = dated(string_field(object, "published"), string_field(object, "updated"));
    Some(NewItem {
        external_id: id,
        url: string_field(object, "url"),
        title: string_field(object, "name"),
        summary: string_field(object, "summary"),
        content: string_field(object, "content"),
        author: actor,
        published_at,
        date_source,
        tags,
        raw: Some(activity.clone()),
        visibility: "public".into(),
    })
}

// ---------------------------------------------------------------- actors

/// What the poller and the Follow flow need from a remote actor document.
#[derive(Debug, Clone)]
pub struct Actor {
    pub id: String,
    pub inbox: String,
    pub outbox: String,
    pub name: Option<String>,
}

/// Poller configuration stored in `source_configs` for `activitypub` sources.
/// `actor_url` is resolved once (at source creation) so polls never depend on
/// WebFinger staying up. The same JSON object carries a `following` flag
/// (`"requested"`, then `true` on Accept), which is merged as raw JSON by
/// `send_follow`/`mark_following_for_actor` rather than modeled here.
#[derive(Debug, Default, Deserialize)]
struct ApConfig {
    #[serde(default)]
    actor_url: Option<String>,
}

/// Resolve a handle (`user@host`, with or without `acct:`) or a direct actor
/// URL into an actor document.
pub async fn resolve_actor(http: &Client, handle_or_url: &str, max_bytes: usize) -> Result<Actor> {
    let trimmed = handle_or_url.trim().trim_start_matches("acct:").to_owned();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return fetch_actor(http, &trimmed, max_bytes).await;
    }
    let (user, host) = trimmed
        .split_once('@')
        .ok_or_else(|| Error::Invalid("expected user@host or an actor URL".into()))?;
    if user.is_empty() || host.is_empty() {
        return Err(Error::Invalid("expected user@host or an actor URL".into()));
    }
    let webfinger = Url::parse(&format!("https://{host}/.well-known/webfinger?resource=acct:{user}@{host}"))
        .map_err(|e| Error::Invalid(format!("invalid webfinger URL: {e}")))?;
    validate_public_url(&webfinger).await?;
    let response = http.get(webfinger).header("Accept", "application/jrd+json").send().await?;
    if !response.status().is_success() {
        return Err(Error::Invalid(format!("webfinger returned {}", response.status())));
    }
    let document: Value = response.json().await.map_err(|e| Error::Invalid(format!("invalid webfinger document: {e}")))?;
    let href = document
        .get("links")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|link| link.get("rel").and_then(Value::as_str) == Some("self"))
        .filter_map(|link| link.get("href").and_then(Value::as_str))
        .next()
        .ok_or_else(|| Error::Invalid("webfinger has no actor link".into()))?;
    fetch_actor(http, href, max_bytes).await
}

/// Fetch one actor document. Every fetch is SSRF-checked and size-bounded
/// like any other ingestion request.
pub async fn fetch_actor(http: &Client, url: &str, max_bytes: usize) -> Result<Actor> {
    let parsed = Url::parse(url).map_err(|e| Error::Invalid(format!("invalid actor URL: {e}")))?;
    validate_public_url(&parsed).await?;
    let document = get_json(http, &parsed, None, max_bytes).await?.ok_or_else(|| Error::Invalid("actor fetch was not modified".into()))?.0;
    actor_from_doc(&document)
}

fn actor_from_doc(document: &Value) -> Result<Actor> {
    let id = document.get("id").and_then(Value::as_str).ok_or_else(|| Error::Invalid("actor has no id".into()))?;
    let inbox = document.get("inbox").and_then(Value::as_str).ok_or_else(|| Error::Invalid("actor has no inbox".into()))?;
    let outbox = document.get("outbox").and_then(Value::as_str).ok_or_else(|| Error::Invalid("actor has no outbox".into()))?;
    let name = document
        .get("preferredUsername")
        .or_else(|| document.get("name"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok(Actor { id: id.to_owned(), inbox: inbox.to_owned(), outbox: outbox.to_owned(), name })
}

/// GET one JSON document bounded by the caller's response cap. `etag` enables a
/// conditional request; `Ok(None)` is a 304 Not Modified.
async fn get_json(http: &Client, url: &Url, etag: Option<&str>, max_bytes: usize) -> Result<Option<(Value, Option<String>)>> {
    let mut request = http.get(url.clone()).header("Accept", ACTIVITY_ACCEPT);
    if let Some(value) = etag {
        request = request.header(IF_NONE_MATCH, value);
    }
    let response = request.send().await?;
    if response.status() == StatusCode::NOT_MODIFIED {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(Error::Invalid(format!("origin returned {}", response.status())));
    }
    let etag_out = response.headers().get(ETAG).and_then(|v| v.to_str().ok()).map(str::to_owned);
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len() + chunk.len() > max_bytes {
            return Err(Error::Invalid("actor response is too large".into()));
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| Error::Invalid(format!("invalid JSON document: {e}")))?;
    Ok(Some((value, etag_out)))
}

fn next_page_url(page: &Value) -> Option<String> {
    match page.get("next") {
        Some(Value::String(url)) => Some(url.clone()),
        Some(object) if object.is_object() => object.get("id").and_then(Value::as_str).map(str::to_owned),
        _ => None,
    }
}

/// Poll one `activitypub` source: fetch the actor, walk its outbox up to
/// [`MAX_OUTBOX_PAGES`] pages, and upsert what `Create`/`Announce` entries
/// carry. Conditional GET reuses the source's stored ETag for the first hop.
pub async fn poll_actor(state: &AppState, source: &Source) -> Result<usize> {
    let stored = state.store.source_config(&source.id).await?.ok_or_else(|| Error::Invalid("activitypub source has no config".into()))?;
    let config: ApConfig = serde_json::from_str(&stored.config_json).map_err(|e| Error::Invalid(format!("invalid activitypub config: {e}")))?;
    let actor_url = config.actor_url.ok_or_else(|| Error::Invalid("activitypub config needs actor_url".into()))?;
    let actor = fetch_actor(&state.http, &actor_url, state.config.max_response_bytes).await?;
    let next_poll = (Utc::now() + chrono::Duration::from_std(state.config.fetch_interval).unwrap_or(chrono::Duration::minutes(15))).to_rfc3339();

    let mut next: Option<String> = Some(actor.outbox.clone());
    let mut etag = source.etag.clone();
    let mut count = 0;
    for _ in 0..MAX_OUTBOX_PAGES {
        let url_str = next.take().unwrap_or_default();
        if url_str.is_empty() {
            break;
        }
        let url = Url::parse(&url_str).map_err(|e| Error::Invalid(format!("invalid outbox URL: {e}")))?;
        validate_public_url(&url).await?;
        match get_json(&state.http, &url, etag.as_deref(), state.config.max_response_bytes).await? {
            None => {
                state.store.update_source_fetch(&source.id, actor.name.as_deref(), None, None, None, &next_poll).await?;
                return Ok(count);
            }
            Some((page, fresh_etag)) => {
                etag = fresh_etag.or(etag);
                let mut page_items = items_from_outbox(&page, &url)?;
                if count + page_items.len() > MAX_OUTBOX_ITEMS_TOTAL {
                    let kept = MAX_OUTBOX_ITEMS_TOTAL.saturating_sub(count);
                    tracing::warn!(kept = kept, total = count + page_items.len(), "outbox poll exceeds total cap; dropping excess");
                    page_items.truncate(kept);
                }
                for item in page_items {
                    let stored = state.store.upsert_item(Some(&source.id), &item).await?;
                    let _ = state.events.send(stored);
                    count += 1;
                }
                if count >= MAX_OUTBOX_ITEMS_TOTAL {
                    break;
                }
                next = next_page_url(&page);
            }
        }
    }
    state.store.update_source_fetch(&source.id, actor.name.as_deref(), etag.as_deref(), None, None, &next_poll).await?;
    Ok(count)
}

// ---------------------------------------------------------------- node identity

/// Canonical id of this node's actor.
pub fn node_actor_id(config: &crate::config::Config) -> String {
    format!("{}/ap/v1/actor", config.public_url.trim_end_matches('/'))
}

fn node_key_id(config: &crate::config::Config) -> String {
    format!("{}#main-key", node_actor_id(config))
}

/// Load the node ed25519 seed, generating and persisting one on first use.
pub async fn node_signing_key(store: &Store) -> Result<SigningKey> {
    if let Some(stored) = store.get_meta("node_signing_key").await? {
        let raw = hex::decode(&stored).map_err(|_| Error::Internal("stored node key is corrupt".into()))?;
        let seed: [u8; 32] = raw.try_into().map_err(|_| Error::Internal("stored node key is corrupt".into()))?;
        return Ok(SigningKey::from_bytes(&seed));
    }
    // The RNG is scoped so it is dropped before any await: `ThreadRng` must
    // not be held across one, or the caller's future stops being `Send`.
    let key = {
        let mut entropy = rand::thread_rng();
        SigningKey::generate(&mut entropy)
    };
    store.set_meta("node_signing_key", &hex::encode(key.to_bytes())).await?;
    Ok(key)
}

/// Fixed 12-byte prefix of an ed25519 SubjectPublicKeyInfo; the key itself is
/// just appended, so no ASN.1 writer is needed to serve a real PEM.
const ED25519_SPKI_PREFIX: [u8; 12] = [0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];

/// Render a verifying key as a `PUBLIC KEY` PEM for the actor document.
pub fn public_key_pem(key: &VerifyingKey) -> String {
    let mut der = Vec::with_capacity(44);
    der.extend_from_slice(&ED25519_SPKI_PREFIX);
    der.extend_from_slice(key.as_bytes());
    let mut pem = String::from("-----BEGIN PUBLIC KEY-----\n");
    for chunk in STANDARD.encode(&der).as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(chunk).expect("base64 is ASCII"));
        pem.push('\n');
    }
    pem.push_str("-----END PUBLIC KEY-----\n");
    pem
}

/// Parse the SPKI PEM back. Anything else (notably RSA PEMs, the Mastodon
/// default) is a clear error rather than a silent mismatch.
pub fn parse_ed25519_pem(pem: &str) -> Result<VerifyingKey> {
    let body: String = pem.lines().filter(|line| !line.starts_with("-----")).collect();
    let der = STANDARD.decode(body.trim()).map_err(|_| Error::Invalid("actor key is not valid PEM".into()))?;
    if der.len() != 44 || der[..12] != ED25519_SPKI_PREFIX {
        return Err(Error::Invalid("actor key is not an ed25519 public key".into()));
    }
    let mut raw = [0u8; 32];
    raw.copy_from_slice(&der[12..]);
    VerifyingKey::from_bytes(&raw).map_err(|e| Error::Invalid(format!("actor key is invalid: {e}")))
}

fn actor_key(document: &Value) -> Result<VerifyingKey> {
    let pem = document
        .get("publicKey")
        .and_then(|key| key.get("publicKeyPem"))
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Invalid("actor has no public key".into()))?;
    parse_ed25519_pem(pem)
}

/// The served actor document: just enough for a remote server to address this
/// node and check its signatures. No outbox collection is advertised because
/// none is served; this node reads, it does not publish.
pub async fn node_actor_doc(state: &AppState) -> Result<Value> {
    let key = node_signing_key(&state.store).await?;
    let id = node_actor_id(&state.config);
    Ok(json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": id,
        "type": "Application",
        "preferredUsername": "sidefeed",
        "name": "Sidefeed",
        "url": state.config.public_url,
        "inbox": format!("{}/ap/v1/inbox", state.config.public_url.trim_end_matches('/')),
        "publicKey": {
            "id": node_key_id(&state.config),
            "owner": id,
            "publicKeyPem": public_key_pem(&key.verifying_key()),
        },
    }))
}

/// WebFinger resource for this node: `acct:sidefeed@<host>` or the actor URL.
pub fn webfinger_doc(state: &AppState, resource: &str) -> Result<Value> {
    let host = Url::parse(&state.config.public_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_default();
    let id = node_actor_id(&state.config);
    let subject = format!("acct:sidefeed@{host}");
    if resource == subject || resource == id {
        Ok(json!({
            "subject": subject,
            "links": [{"rel": "self", "type": "application/activity+json", "href": id}],
        }))
    } else {
        Err(Error::NotFound)
    }
}

// ---------------------------------------------------------------- follow + inbox signatures

fn http_date_now() -> String {
    Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string()
}

fn parse_http_date(value: &str) -> Result<chrono::DateTime<Utc>> {
    // Inbox dates are IMF-fixdate, the RFC2822 profile HTTP mandates.
    chrono::DateTime::parse_from_rfc2822(value)
        .map(|stamp| stamp.with_timezone(&Utc))
        .map_err(|_| Error::Invalid("unparseable Date header".into()))
}

struct ParsedSignature {
    key_id: String,
    headers: Vec<String>,
    signature: Vec<u8>,
}

fn parse_signature_header(value: &str) -> Result<ParsedSignature> {
    let mut key_id = None;
    let mut headers = None;
    let mut signature = None;
    for mut part in value.split(',') {
        part = part.trim();
        let (name, quoted) = part.split_once('=').ok_or_else(|| Error::Invalid("malformed Signature header".into()))?;
        let val = quoted.trim().trim_matches('"').to_owned();
        match name.trim() {
            "keyId" => key_id = Some(val),
            "headers" => headers = Some(val),
            "signature" => signature = Some(val),
            _ => {}
        }
    }
    Ok(ParsedSignature {
        key_id: key_id.ok_or_else(|| Error::Invalid("Signature header needs keyId".into()))?,
        headers: headers
            .ok_or_else(|| Error::Invalid("Signature header needs headers".into()))?
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
        signature: STANDARD
            .decode(signature.ok_or_else(|| Error::Invalid("Signature header needs signature".into()))?)
            .map_err(|_| Error::Invalid("Signature header is not base64".into()))?,
    })
}

/// Rebuild the exact bytes a signer signed, from the header names they list.
fn signing_string(headers: &HeaderMap, names: &[String], method: &str, target: &str, body: &[u8]) -> Result<String> {
    let mut lines = Vec::new();
    for name in names {
        match name.as_str() {
            "(request-target)" => lines.push(format!("(request-target): {method} {target}")),
            "host" => lines.push(format!("host: {}", header_value(headers, "host")?)),
            "date" => lines.push(format!("date: {}", header_value(headers, "date")?)),
            "digest" => {
                let expected = format!("SHA-256={}", STANDARD.encode(Sha256::digest(body)));
                if header_value(headers, "digest")? != expected {
                    return Err(Error::Unauthorized);
                }
                lines.push(format!("digest: {expected}"));
            }
            other => return Err(Error::Invalid(format!("unsupported signed header: {other}"))),
        }
    }
    Ok(lines.join("\n"))
}

fn header_value(headers: &HeaderMap, name: &str) -> Result<String> {
    headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_owned).ok_or(Error::Unauthorized)
}

/// Verify an inbound delivery against the claimed actor key, including the
/// digest when the signer covered it and the 5-minute date skew window.
/// A non-empty body must be covered by `digest`; digest-less signed
/// deliveries with bodies are rejected outright.
pub fn verify_request_signature(key: &VerifyingKey, headers: &HeaderMap, method: &str, target: &str, body: &[u8]) -> Result<()> {
    let raw = headers.get("signature").and_then(|v| v.to_str().ok()).ok_or(Error::Unauthorized)?;
    let parsed = parse_signature_header(raw)?;
    if !parsed.headers.iter().any(|name| name == "date") {
        return Err(Error::Invalid("Signature must cover the date header".into()));
    }
    if !body.is_empty() && !parsed.headers.iter().any(|name| name == "digest") {
        return Err(Error::Unauthorized);
    }
    let date = parse_http_date(&header_value(headers, "date")?)?;
    if (Utc::now() - date).num_seconds().abs() > INBOX_SKEW_SECONDS {
        return Err(Error::Unauthorized);
    }
    let text = signing_string(headers, &parsed.headers, method, target, body)?;
    let bytes: [u8; 64] = parsed.signature.try_into().map_err(|_| Error::Invalid("Signature is not 64 bytes".into()))?;
    key.verify(text.as_bytes(), &Signature::from_bytes(&bytes)).map_err(|_| Error::Unauthorized)
}

/// POST one signed activity to a remote inbox: Date + Digest + Signature over
/// `(request-target) host date digest`, the same shape the inbox verifies.
async fn signed_post(http: &Client, key: &SigningKey, key_id: &str, url: &Url, body: Vec<u8>) -> Result<()> {
    let digest = format!("SHA-256={}", STANDARD.encode(Sha256::digest(&body)));
    let date = http_date_now();
    let host = match url.port() {
        Some(port) => format!("{}:{port}", url.host_str().unwrap_or("")),
        None => url.host_str().unwrap_or("").to_owned(),
    };
    let mut target = format!("post {}", url.path());
    if let Some(query) = url.query() {
        target.push('?');
        target.push_str(query);
    }
    let text = format!("(request-target): {target}\nhost: {host}\ndate: {date}\ndigest: {digest}");
    let signature = STANDARD.encode(key.sign(text.as_bytes()).to_bytes());
    let header = format!("keyId=\"{key_id}\",algorithm=\"hs2019\",headers=\"(request-target) host date digest\",signature=\"{signature}\"");
    let response = http
        .post(url.clone())
        .header("Date", date)
        .header("Digest", digest)
        .header("Signature", header)
        .header("Content-Type", "application/activity+json")
        .body(body)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(Error::Invalid(format!("actor inbox returned {}", response.status())));
    }
    Ok(())
}

/// Send a Follow to the actor behind one source and record it as requested.
/// The matching Accept arrives later at the inbox and flips it to `true`.
pub async fn send_follow(state: &AppState, source: &Source) -> Result<Value> {
    let stored = state.store.source_config(&source.id).await?.ok_or_else(|| Error::Invalid("activitypub source has no config".into()))?;
    let config: ApConfig = serde_json::from_str(&stored.config_json).map_err(|e| Error::Invalid(format!("invalid activitypub config: {e}")))?;
    let actor_url = config.actor_url.ok_or_else(|| Error::Invalid("activitypub config needs actor_url".into()))?;
    let actor = fetch_actor(&state.http, &actor_url, state.config.max_response_bytes).await?;
    let key = node_signing_key(&state.store).await?;
    let follow = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{}/ap/v1/follows/{}", state.config.public_url.trim_end_matches('/'), Uuid::new_v4()),
        "type": "Follow",
        "actor": node_actor_id(&state.config),
        "object": actor.id,
    });
    let inbox = Url::parse(&actor.inbox).map_err(|e| Error::Invalid(format!("invalid actor inbox: {e}")))?;
    validate_public_url(&inbox).await?;
    signed_post(&state.http, &key, &node_key_id(&state.config), &inbox, serde_json::to_vec(&follow).map_err(|e| Error::Internal(e.to_string()))?).await?;
    let mut merged: Value = serde_json::from_str(&stored.config_json).unwrap_or(json!({}));
    merged["actor_url"] = Value::String(actor.id);
    merged["following"] = Value::String("requested".into());
    state.store.put_source_config(&source.id, "activitypub", &merged.to_string()).await?;
    Ok(follow)
}

// ---------------------------------------------------------------- inbox

/// Verify and dispatch one inbox delivery. `Accept{Follow}` flips the matching
/// source config to `following:true`; `Create` normalizes one item from an
/// actor this node tracks and stores nothing from strangers.
///
/// The `Signature` keyId owner must equal `activity.actor`; a delivery that
/// claims to be from a tracked victim but is signed by another actor's key
/// is rejected as 401 before any fetch or store.
/// Whether the `Signature` keyId owner matches `activity.actor`. Both must be
/// present and byte-equal; anything else is an impersonation attempt.
pub(crate) fn binding_matches(key_id_owner: &str, activity: &Value) -> bool {
    match activity.get("actor").and_then(Value::as_str) {
        Some(actor) if !actor.is_empty() => actor == key_id_owner,
        _ => false,
    }
}

pub async fn handle_inbox(state: &AppState, headers: &HeaderMap, body: &[u8]) -> Result<Value> {
    let activity: Value = serde_json::from_slice(body).map_err(|_| Error::Invalid("inbox body must be JSON".into()))?;
    let raw = headers.get("signature").and_then(|v| v.to_str().ok()).ok_or(Error::Unauthorized)?;
    let parsed = parse_signature_header(raw)?;
    let actor_url = parsed.key_id.split('#').next().unwrap_or("").to_owned();
    if !binding_matches(&actor_url, &activity) {
        return Err(Error::Unauthorized);
    }
    let url = Url::parse(&actor_url).map_err(|_| Error::Invalid("Signature keyId is not a URL".into()))?;
    validate_public_url(&url).await?;
    let document = get_json(&state.http, &url, None, state.config.max_response_bytes).await?.ok_or_else(|| Error::Invalid("actor fetch was not modified".into()))?.0;
    verify_request_signature(&actor_key(&document)?, headers, "post", "/ap/v1/inbox", body)?;
    match activity.get("type").and_then(Value::as_str).unwrap_or("") {
        "Accept" => handle_accept(state, &activity).await,
        "Create" => handle_create(state, &activity).await,
        _ => Ok(json!({"accepted": true, "stored": 0})),
    }
}

async fn handle_accept(state: &AppState, activity: &Value) -> Result<Value> {
    let object = activity.get("object").unwrap_or(&Value::Null);
    if object.get("type").and_then(Value::as_str) != Some("Follow") {
        return Ok(json!({"accepted": true, "stored": 0}));
    }
    // The Follow's actor must be this node and its object the followed actor;
    // anything else is someone else's conversation.
    if object.get("actor").and_then(Value::as_str) != Some(&node_actor_id(&state.config)) {
        return Ok(json!({"accepted": true, "stored": 0}));
    }
    let followed = match object.get("object") {
        Some(Value::String(id)) => id.clone(),
        Some(object) if object.is_object() => object.get("id").and_then(Value::as_str).unwrap_or("").to_owned(),
        _ => String::new(),
    };
    let sender = activity.get("actor").and_then(Value::as_str).unwrap_or("");
    if followed != sender || followed.is_empty() {
        return Ok(json!({"accepted": true, "stored": 0}));
    }
    let marked = mark_following_for_actor(&state.store, &followed).await?;
    Ok(json!({"accepted": true, "following": marked}))
}

async fn handle_create(state: &AppState, activity: &Value) -> Result<Value> {
    let object = activity.get("object").filter(|v| v.is_object()).unwrap_or(&Value::Null);
    let base = Url::parse(activity.get("actor").and_then(Value::as_str).unwrap_or("https://localhost/"))
        .unwrap_or_else(|_| Url::parse("https://localhost/").expect("literal parses"));
    let Some(item) = normalize_object(object, activity, &base) else {
        return Ok(json!({"accepted": true, "stored": 0}));
    };
    // Only actors this node tracks may write into the store through the inbox,
    // authorized on the verified sender alone. `attributedTo` is author
    // metadata, not authority: trusting it would let any signer store items
    // under a tracked victim by naming them as the attributed author.
    let sender = activity.get("actor").and_then(Value::as_str).unwrap_or("");
    let source_id = source_for_actor(&state.store, &[sender]).await?;
    let Some(source_id) = source_id else {
        return Ok(json!({"accepted": true, "stored": 0}));
    };
    let stored = state.store.upsert_item(Some(&source_id), &item).await?;
    let _ = state.events.send(stored);
    Ok(json!({"accepted": true, "stored": 1}))
}

/// The source tracking one of these actor ids, if any.
async fn source_for_actor(store: &Store, actors: &[&str]) -> Result<Option<String>> {
    for stored in store.source_configs_by_kind("activitypub").await? {
        let config: ApConfig = serde_json::from_str(&stored.config_json).unwrap_or_default();
        if let Some(url) = config.actor_url {
            if actors.contains(&url.as_str()) {
                return Ok(Some(stored.source_id));
            }
        }
    }
    Ok(None)
}

/// Flip the source tracking this actor to `following:true`. Returns whether a
/// source matched, so callers can report it honestly.
pub async fn mark_following_for_actor(store: &Store, actor_url: &str) -> Result<bool> {
    for stored in store.source_configs_by_kind("activitypub").await? {
        let mut merged: Value = serde_json::from_str(&stored.config_json).unwrap_or(json!({}));
        if merged.get("actor_url").and_then(Value::as_str) == Some(actor_url) {
            merged["following"] = Value::Bool(true);
            store.put_source_config(&stored.source_id, "activitypub", &merged.to_string()).await?;
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn signed_headers(key: &SigningKey, body: &[u8]) -> HeaderMap {
        let digest = format!("SHA-256={}", STANDARD.encode(Sha256::digest(body)));
        let date = http_date_now();
        let text = format!("(request-target): post /ap/v1/inbox\nhost: sidefeed.test\ndate: {date}\ndigest: {digest}");
        let signature = STANDARD.encode(key.sign(text.as_bytes()).to_bytes());
        [
            ("host", "sidefeed.test"),
            ("date", &date),
            ("digest", &digest),
            (
                "signature",
                &format!(
                    "keyId=\"https://sidefeed.test/ap/v1/actor#main-key\",headers=\"(request-target) host date digest\",signature=\"{signature}\""
                ),
            ),
        ]
        .into_iter()
        .map(|(name, value)| (name.parse().unwrap(), value.parse().unwrap()))
        .collect()
    }

    #[test]
    fn node_key_survives_a_pem_round_trip_and_signatures_verify() {
        let key = test_key();
        let parsed = parse_ed25519_pem(&public_key_pem(&key.verifying_key())).unwrap();
        assert_eq!(parsed.as_bytes(), key.verifying_key().as_bytes());

        let body = br#"{"type":"Accept"}"#;
        let headers = signed_headers(&key, body);
        assert!(verify_request_signature(&key.verifying_key(), &headers, "post", "/ap/v1/inbox", body).is_ok());
        // A tampered body breaks the digest check before crypto even runs.
        assert!(verify_request_signature(&key.verifying_key(), &headers, "post", "/ap/v1/inbox", br#"{"type":"Forged"}"#).is_err());
    }

    #[test]
    fn rsa_pems_are_rejected_with_a_clear_error() {
        let pem = "-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKC\n-----END PUBLIC KEY-----\n";
        let error = parse_ed25519_pem(pem).unwrap_err().to_string();
        assert!(error.contains("ed25519"), "unexpected error: {error}");
    }

    #[test]
    fn outbox_pages_accept_bare_objects_and_reject_likes() {
        let base = Url::parse("https://m.example/users/jo/outbox").unwrap();
        let page = json!({"orderedItems": [
            {"type": "Note", "id": "https://m.example/p/0", "content": "bare"},
            {"type": "Like", "actor": "https://m.example/users/jo",
             "object": {"type": "Note", "id": "https://m.example/p/9", "content": "liked"}},
        ]});
        let items = items_from_outbox(&page, &base).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].external_id, "https://m.example/p/0");
    }

    #[test]
    fn digest_is_required_when_the_body_is_non_empty() {
        let key = test_key();
        let body = br#"{"type":"Create"}"#;
        // Headers signed without `digest` must be rejected when a body is present.
        let date = http_date_now();
        let text = format!("(request-target): post /ap/v1/inbox\nhost: sidefeed.test\ndate: {date}");
        let signature = STANDARD.encode(key.sign(text.as_bytes()).to_bytes());
        let headers: HeaderMap = [
            ("host", "sidefeed.test"),
            ("date", date.as_str()),
            (
                "signature",
                &format!(
                    "keyId=\"https://sidefeed.test/ap/v1/actor#main-key\",headers=\"(request-target) host date\",signature=\"{signature}\""
                ),
            ),
        ]
        .into_iter()
        .map(|(name, value)| (name.parse().unwrap(), value.parse().unwrap()))
        .collect();
        assert!(verify_request_signature(&key.verifying_key(), &headers, "post", "/ap/v1/inbox", body).is_err());
        // The same shape with `digest` covered verifies.
        let headers = signed_headers(&key, body);
        assert!(verify_request_signature(&key.verifying_key(), &headers, "post", "/ap/v1/inbox", body).is_ok());
    }

    #[test]
    fn outbox_pages_truncate_at_two_hundred_items() {
        let base = Url::parse("https://m.example/users/jo/outbox").unwrap();
        let ordered: Vec<Value> = (0..250)
            .map(|i| json!({"type": "Note", "id": format!("https://m.example/p/{i}"), "content": "hello"}))
            .collect();
        let page = json!({"orderedItems": ordered});
        let items = items_from_outbox(&page, &base).unwrap();
        assert_eq!(items.len(), super::MAX_OUTBOX_ITEMS_PER_PAGE);
    }

    #[tokio::test]
    async fn inbox_create_authorizes_sender_not_attributed_author() {
        // A source tracks the victim. An attacker-signed Create naming the
        // victim only as attributedTo must resolve to no source, so nothing
        // is stored under the victim.
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("t.db").display());
        let store = Store::connect(&url).await.unwrap();
        let source = store.create_source("https://victim.example/users/v", "activitypub", None).await.unwrap();
        store.put_source_config(&source.id, "activitypub", r#"{"actor_url":"https://victim.example/users/v"}"#).await.unwrap();
        assert!(super::source_for_actor(&store, &["https://victim.example/users/v"]).await.unwrap().is_some());
        assert!(super::source_for_actor(&store, &["https://attacker.example/users/a"]).await.unwrap().is_none());
    }

    #[test]
    fn inbox_binding_requires_keyid_owner_to_equal_actor() {
        let victim = json!({"type": "Create", "actor": "https://victim.example/users/v"});
        assert!(super::binding_matches("https://victim.example/users/v", &victim));
        // Attacker signs with their own key but claims to be the victim.
        assert!(!super::binding_matches("https://attacker.example/users/a", &victim));
        // Missing actor never matches.
        assert!(!super::binding_matches("https://victim.example/users/v", &json!({"type": "Create"})));
    }
}

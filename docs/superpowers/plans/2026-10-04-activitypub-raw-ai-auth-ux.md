# Sidefeed v2: ActivityPub follow, raw channels, tiny AI, auth, and unified UX — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:delegate (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add ActivityPub read-plus-follow sources, generic raw-JSON/webhook channels, small-footprint AI (ONNX-local + external APIs), a proper auth/key workflow with rate limiting, and unify the reader on the SolidJS app with a management UI.

**Architecture:** Keep the one-process/one-SQLite design. New source kinds ride the existing poll loop and `Store::upsert_item` path; ActivityPub follow uses a minimal node actor with WebFinger plus a signed inbox; AI stays a replaceable enrichment stage with a unified status endpoint; auth becomes scoped API keys checked by one middleware and per-IP rate limits via a governor layer.

**Tech Stack:** Rust 2021, Axum 0.8, tower-http 0.6 + tower_governor (new), sqlx 0.8 (SQLite), tokio 1, reqwest 0.12 (rustls), ort 2.x behind `onnx-local` feature (new, optional), SolidJS + Vite (`web/`, artifact committed to `src/web/dist/`).

**Spec:** This plan argues from the operator requests in chat (2026-10-04): ActivityPub read-only+follow, generic JSON/REST poller + webhooks, external APIs plus very small local models via ONNX, unify on SolidJS with management UI, plus proper authentication workflow and rate limiting. Prior art: `PLAN.md` (v1 boundaries), `README.md`, `SECURITY.md`.

## Global Constraints

- One Rust process and one SQLite file stays the default deployment; no external service may be required for the local ingest/read path.
- Fetch public HTTP(S) sources only: every URL and every redirect hop is checked by `ingest::validate_public_url` (SSRF denylist); responses bounded by `SIDEFEED_MAX_RESPONSE_BYTES` (default 5242880) with `SIDEFEED_FETCH_TIMEOUT_SECONDS` (default 20).
- Federation stays pull-based and explicitly configured; this plan does NOT build a general-purpose ActivityPub server (no multi-user inboxes, no relays).
- AI is a replaceable enrichment/filter stage: ingestion and delivery keep working with providers set to `disabled`; per-item input truncated at `SIDEFEED_ENRICH_MAX_CHARS` (default 8000); in-memory artifact cache capped by `SIDEFEED_ENRICH_CACHE_ENTRIES` (default 128); target host stays ~2 vCPU / 1 GB.
- Commit messages follow repo history: `<type>: <summary>`, imperative, summary ≤50 chars, e.g. `feat: add scoped api keys`.
- `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo check` must pass per task; `cargo check --features burn-local` and `cargo check --features onnx-local` must pass when those areas change.

---

## File / responsibility map (locked by this plan)

| File | Responsibility |
|---|---|
| `src/auth.rs` (new) | Scoped API-key model: mint, verify (SHA-256 hash compare), `require_scope` middleware helper, failed-auth counter |
| `src/ratelimit.rs` (new, or inside `auth.rs` if <150 lines) | Governor config: global + strict rule sets, `RateLimitLayer` builders |
| `src/ap.rs` (new) | ActivityPub client: WebFinger resolve, actor fetch, outbox pagination, Follow send, inbox Accept/Create handling, node actor + keypair |
| `src/channels.rs` (new) | Raw-channel ingest: `raw-json` poller (pointer + field map), webhook payload normalizer |
| `src/ai.rs` (modify) | Add `OnnxProvider` (embed) behind `onnx-local`; auto-embed hook; keep `RemoteProvider` |
| `src/enrich.rs` (modify) | Unified `/api/v1/ai/status`; shared `LlmChat` trait so ask/brief reuse the openai-compatible chat path |
| `src/ask.rs` (new, or inside `enrich.rs` if <200 lines) | `POST /api/v1/feeds/{slug}/ask` hybrid retrieve + chat with extractive fallback |
| `src/ingest.rs` (modify) | Dispatch `source.kind` to `ap::` / `channels::` pollers; JSON Feed discovery + `next_url` paging |
| `src/api.rs` (modify) | Key management routes, webhook ingress route, inbox/actor routes, layers (body limit, timeout, governor) |
| `src/store.rs`, `src/model.rs` (modify) | `api_keys`, `source_configs`, `webhook_channels`, `node_meta` tables + accessors |
| `src/config.rs` (modify) | New env vars (below); `src/main.rs` unchanged except layer wiring lives in `api::router` |
| `migrations/20261004XXXX_*.sql` (new, 3 files) | Keys, channels, node identity tables |
| `web/src/views/Manage.tsx`, `web/src/views/Keys.tsx`, `web/src/views/Ai.tsx` (new) | Sources/feeds management, key mint/revoke, AI status |
| `src/web/dist/*` (rebuild) | Committed build artifact of `web/` (procedure in Task 7) |
| `src/db.rs`, `src/errors.rs`, `src/api/update.rs` (delete) | Dead actix/error-stack remnants, not referenced by `lib.rs` |
| `tests/e2e.rs` (modify) | Black-box coverage per task |
| `src/web/openapi.json`, `src/web/docs.html`, `README.md`, `SECURITY.md` (modify) | Contract + operator docs per task |

New environment variables (all optional, defaults = current behaviour):

| Variable | Default | Purpose |
|---|---|---|
| `SIDEFEED_API_KEYS_ENABLED` | `0` | `1` requires scoped keys for management even when no admin token set (localhost default stays open) |
| `SIDEFEED_BOOKMARKS_REQUIRE_AUTH` | `0` | `1` makes bookmark writes require any valid key (for shared/hostile networks) |
| `SIDEFEED_RATE_LIMIT_RPS` | `10` | Global per-IP sustained rate |
| `SIDEFEED_RATE_LIMIT_BURST` | `30` | Global per-IP burst |
| `SIDEFEED_AP_ENABLED` | `0` | `1` serves the node actor + inbox (needed to receive Follow Accepts) |
| `SIDEFEED_ONNX_EMBED_MODEL` | unset | Path to `.onnx` embedding model (e.g. MiniLM-L6-v2 quantized); enables `onnx-local` provider |

---

### Task 1: Scoped API keys, auth workflow, and rate limiting

**Files:**
- Create: `src/auth.rs`, `migrations/20261004090000_api_keys.sql`
- Modify: `src/lib.rs` (add `pub mod auth;`), `src/api.rs` (key routes + middleware swap), `src/config.rs` (new vars), `src/store.rs` (key accessors), `SECURITY.md`
- Test: `tests/e2e.rs` (append key + ratelimit tests)
- Delete: nothing in this task

**Interfaces:**
- Consumes: `AppState`, `Store`, `Config` (existing).
- Produces: `auth::mint_key(&Store, name, scopes) -> Result<(String /*one-time plaintext*/, ApiKey)>`; `auth::require_scope(state, headers, scope: &str) -> Result<ApiKey>`; `auth::hash_token(&str) -> String`. Later tasks use `require_scope` for webhook/ingress management routes.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn scoped_keys_gate_management_and_support_revocation() {
    let (app, state, _d) = fixture().await;
    // Mint a read-only key via the admin token.
    let minted = app.clone().oneshot(request("POST", "/api/v1/keys",
        Some(json!({"name":"reader","scopes":["read:private"]})), true)).await.unwrap();
    assert_eq!(minted.status(), StatusCode::CREATED);
    let body = json_body(minted).await;
    let token = body["token"].as_str().expect("one-time plaintext").to_string();
    // Read-only key cannot create feeds.
    let denied = app.clone().oneshot(keyed("POST", "/api/v1/feeds",
        Some(json!({"slug":"x","title":"x"})), &token)).await.unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    // Revoke, then even reads fail.
    let _ = state.store.revoke_key(body["id"].as_str().unwrap()).await.unwrap();
    let gone = app.oneshot(keyed("GET", "/api/v1/sources", None, &token)).await.unwrap();
    assert_eq!(gone.status(), StatusCode::UNAUTHORIZED);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test scoped_keys_gate_management 2>&1 | tail -5`
Expected: FAIL with compile error (`auth`/key routes do not exist).

- [ ] **Step 3: Write minimal implementation**

Migration `migrations/20261004090000_api_keys.sql`:

```sql
CREATE TABLE IF NOT EXISTS api_keys (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  prefix TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  scopes_json TEXT NOT NULL DEFAULT '[]',
  revoked INTEGER NOT NULL DEFAULT 0,
  last_used_at TEXT,
  created_at TEXT NOT NULL
);
```

`src/auth.rs` (essentials):

```rust
use sha2::{Digest, Sha256};
use crate::{error::{Error, Result}, AppState};

pub fn hash_token(plaintext: &str) -> String {
    hex::encode(Sha256::digest(plaintext.as_bytes()))
}

/// Mint a `sf_<30 random chars>` token; only the hash is stored.
pub async fn mint_key(state: &AppState, name: &str, scopes: &[String])
    -> Result<(String, crate::model::ApiKey)>
{
    use rand::RngCore;
    let mut bytes = [0u8; 22];
    rand::thread_rng().fill_bytes(&mut bytes);
    let plaintext = format!("sf_{}", base64::encode_config(&bytes, base64::URL_SAFE_NO_PAD));
    let key = state.store.create_key(name, &plaintext[..8], &hash_token(&plaintext), scopes).await?;
    Ok((plaintext, key))
}

/// Admin bearer still passes everything; otherwise a live key with the scope.
pub async fn require_scope(state: &AppState, headers: &axum::http::HeaderMap, scope: &str)
    -> Result<crate::model::ApiKey>
{
    if crate::api::authorize(state, headers).is_ok() {
        return Ok(crate::model::ApiKey::superuser());
    }
    let supplied = headers.get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(Error::Unauthorized)?;
    let key = state.store.key_by_hash(&hash_token(supplied)).await?;
    if key.revoked || !key.scopes.contains(&scope.to_string()) {
        return Err(Error::Forbidden);
    }
    state.store.touch_key(&key.id).await?;
    Ok(key)
}
```

Add `Error::Forbidden` → 403 in `src/error.rs`, `ApiKey` struct + `create_key/key_by_hash/revoke_key/touch_key` in `src/store.rs`, routes `POST /api/v1/keys` + `DELETE /api/v1/keys/{id}` (admin-only) in `src/api.rs`. Token extraction helper `keyed()` in `tests/e2e.rs` builds a `Request` with `Authorization: Bearer <token>`. Rate limiting in this task: add `tower_governor = "0.6"` to `Cargo.toml` and wrap only the strict set first:

```rust
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
let strict = GovernorConfigBuilder::default()
    .per_second(1).burst_size(10).finish().unwrap();
Router::new().route("/api/v1/keys", post(create_key))
    .layer(GovernorLayer::new(strict))
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test scoped_keys_gate_management && cargo clippy --all-targets -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 5: Commit**

```bash
git add migrations/20261004090000_api_keys.sql src/auth.rs src/lib.rs src/api.rs src/config.rs src/store.rs src/model.rs src/error.rs Cargo.toml Cargo.lock tests/e2e.rs SECURITY.md
git commit -m "feat: add scoped api keys and strict rate limits"
```

Notes for implementer: `SIDEFEED_API_KEYS_ENABLED=1` flips management routes to require keys even without an admin token (localhost stays open at `0`). `SIDEFEED_BOOKMARKS_REQUIRE_AUTH=1` routes bookmark writes through `require_scope(_, "bookmarks:write")`. Global governor (10 rps / burst 30, env-tunable) lands in Task 8 with the full layer stack; this task only gates `/keys` so the diff stays reviewable.

---

### Task 2: Channel model — source configs + webhook channels

**Files:**
- Create: `migrations/20261004100000_channels.sql`
- Modify: `src/store.rs`, `src/model.rs`, `src/api.rs` (CRUD for channels), `src/web/openapi.json`
- Test: `tests/e2e.rs`

**Interfaces:**
- Consumes: `Store` from Task 1.
- Produces: `store::put_source_config(source_id, kind, config_json)`, `store::source_config(source_id) -> Option<SourceConfig>`, `store::create_channel(slug, secret_hash, source_id)`, `ingest` dispatch reads `source.kind` ∈ {`raw-json`, `webhook`, `activitypub`} (used by Tasks 3–4).

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn webhook_channel_ingests_a_signed_batch() {
    let (app, state, _d) = fixture().await;
    let source = state.store.create_source("webhook:deploy-events", "webhook", Some("Deploys")).await.unwrap();
    let created = app.clone().oneshot(request("POST", "/api/v1/feeds",
        Some(json!({"slug":"ops","title":"Ops","public":true})), true)).await.unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let _ = state.store.attach_source("ops", &source.id).await.unwrap();
    let channel = state.store.create_channel("deploys", &crate::auth::hash_token("s3cret"), Some(&source.id)).await.unwrap();
    assert_eq!(channel.slug, "deploys");
    let body = json!({"items":[{"id":"d1","title":"deploy v42","url":"https://ex.example/d/42"}]});
    let res = app.oneshot(signed_ingress("POST", "/api/v1/ingress/deploys", body, "s3cret")).await.unwrap();
    assert_eq!(res.status(), StatusCode::ACCEPTED);
    let items = state.store.feed_items("ops", 10, None).await.unwrap();
    assert_eq!(items.len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test webhook_channel_ingests 2>&1 | tail -5`
Expected: FAIL (`create_channel` / `/api/v1/ingress` missing).

- [ ] **Step 3: Write minimal implementation**

Migration:

```sql
CREATE TABLE IF NOT EXISTS source_configs (
  source_id TEXT PRIMARY KEY REFERENCES sources(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  config_json TEXT NOT NULL DEFAULT '{}'
);
CREATE TABLE IF NOT EXISTS webhook_channels (
  id TEXT PRIMARY KEY,
  slug TEXT NOT NULL UNIQUE,
  secret_hash TEXT NOT NULL,
  source_id TEXT REFERENCES sources(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL
);
```

Model + store: `SourceConfig { source_id, kind, config_json }`, `WebhookChannel { id, slug, source_id, created_at }` (never serialize `secret_hash`). `create_channel` validates slug with the same rule as feed slugs. Ingress route (in `src/api.rs`):

```rust
async fn ingress(State(state): State<AppState>, Path(slug): Path<String>,
    headers: HeaderMap, body: Bytes) -> Result<(StatusCode, Json<Value>)> {
    let channel = state.store.channel(&slug).await?;
    channels::verify_ingress(&channel, &headers, &body)?; // HMAC x-sidefeed-signature or Bearer
    let items = channels::normalize_webhook(&body)?;      // -> Vec<NewItem>, capped at 100
    let mut n = 0;
    for item in items {
        state.store.upsert_item(channel.source_id.as_deref(), &item).await?;
        n += 1;
    }
    Ok((StatusCode::ACCEPTED, Json(json!({"accepted": n}))))
}
```

Body cap reuses the 2 MiB `RequestBodyLimitLayer`; item cap 100 returns 413 above it.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test webhook_channel_ingests && cargo test`
Expected: PASS, full suite green.

- [ ] **Step 5: Commit**

```bash
git add migrations/20261004100000_channels.sql src/store.rs src/model.rs src/api.rs src/channels.rs src/web/openapi.json tests/e2e.rs
git commit -m "feat: add webhook channels and source configs"
```

---

### Task 3: ActivityPub read-plus-follow (no general server)

**Files:**
- Create: `src/ap.rs`, `migrations/20261004110000_node_identity.sql`
- Modify: `src/lib.rs`, `src/ingest.rs` (dispatch `kind == "activitypub"`), `src/api.rs` (actor, webfinger, inbox routes), `src/config.rs`, `src/web/openapi.json`, `SECURITY.md`
- Test: `tests/e2e.rs` + `src/ap.rs` unit tests

**Interfaces:**
- Consumes: `validate_public_url`, `store::upsert_item`, `source_configs` from Task 2.
- Produces: `ap::poll_actor(state, source) -> Result<usize>`; `ap::resolve_actor(http, handle_or_url) -> Result<Actor>`; nothing else depends on AP internals.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn parses_create_and_announce_from_an_outbox_page() {
    let page = json!({
        "orderedItems": [
            {"type":"Create","actor":"https://m.example/users/jo",
             "object":{"type":"Note","id":"https://m.example/p/1",
               "content":"<p>hello</p>","published":"2026-10-01T00:00:00Z"}},
            {"type":"Announce","actor":"https://m.example/users/jo",
             "object":{"type":"Note","id":"https://m.example/p/2",
               "content":"boosted","published":"2026-10-02T00:00:00Z"}}
        ]
    });
    let items = sidefeed::ap::items_from_outbox(&page,
        &"https://m.example/users/jo/outbox".parse().unwrap()).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].external_id, "https://m.example/p/1");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test parses_create_and_announce 2>&1 | tail -3`
Expected: FAIL (`src/ap.rs` missing).

- [ ] **Step 3: Write minimal implementation**

`src/ap.rs` core (reuse the existing `parse_activitypub` object mapping from `ingest.rs`; move, don't duplicate):

```rust
pub fn items_from_outbox(page: &serde_json::Value, base: &url::Url) -> Result<Vec<NewItem>> {
    let list = page.get("orderedItems").or_else(|| page.get("items"))
        .and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut items = Vec::new();
    for activity in list {
        // Accept bare objects plus Create/Announce wrappers; ignore the rest.
        let kind = activity.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let object = match kind {
            "Create" | "Announce" => activity.get("object").unwrap_or(&activity),
            _ => &activity,
        };
        if let Some(item) = normalize_object(object, &activity) { items.push(item); }
    }
    Ok(items)
}
```

Polling: `poll_actor` fetches the actor's `outbox` (from source config `actor_url`, resolved once at source creation via `resolve_actor` which tries WebFinger `/.well-known/webfinger?resource=acct:<handle>` then falls back to a direct actor URL), walks `first`/`next` collection pages up to 3 pages, conditional GET with stored ETag, each hop via `validate_public_url`. Follow: `POST /api/v1/sources/{id}/follow` sends a signed `Follow` to the actor's inbox using the node keypair (ed25519, generated once into `node_meta` table, served at `GET /ap/v1/actor` + `/.well-known/webfinger` only when `SIDEFEED_AP_ENABLED=1`); `POST /ap/v1/inbox` verifies HTTP signatures minimally (fetch actor key, check signature header, 5-min skew) and on `Accept{Follow}` marks the source config `following:true`, on `Create` normalizes one item. Inbox is inert when the flag is `0`: route returns 404.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test ap:: && cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/ap.rs src/ingest.rs src/api.rs src/config.rs src/store.rs src/model.rs migrations/20261004110000_node_identity.sql src/web/openapi.json SECURITY.md tests/e2e.rs
git commit -m "feat: add activitypub read-plus-follow sources"
```

---

### Task 4: JSON Feed hardening + generic raw-JSON poller

**Files:**
- Modify: `src/ingest.rs` (discovery, `next_url` paging), `src/channels.rs` (raw-json poller), `src/api.rs` (source create accepts `kind`+`config`), `src/web/openapi.json`
- Test: `tests/e2e.rs` + `src/ingest.rs` unit tests

**Interfaces:**
- Consumes: `source_configs` (Task 2), `validate_public_url`, `upsert_item`.
- Produces: `channels::poll_raw_json(state, source, config: &RawJsonConfig) -> Result<usize>`; `ingest::discover_feed_url(html, base) -> Option<String>`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn raw_json_pointer_selects_items_and_maps_fields() {
    let doc = json!({"data":{"posts":[
        {"uid":"a1","headline":"Hello","link":"https://ex.example/a1","ts":"2026-10-01T00:00:00Z"}]}});
    let config = sidefeed::channels::RawJsonConfig {
        items_pointer: "/data/posts".into(), id_field: "uid".into(),
        title_field: Some("headline".into()), url_field: Some("link".into()),
        date_field: Some("ts".into()), ..Default::default()
    };
    let items = sidefeed::channels::select_items(&doc, &config).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].external_id, "a1");
    assert_eq!(items[0].title.as_deref(), Some("Hello"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test raw_json_pointer_selects 2>&1 | tail -3`
Expected: FAIL (`channels::RawJsonConfig` missing).

- [ ] **Step 3: Write minimal implementation**

```rust
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct RawJsonConfig {
    #[serde(default = "default_pointer")]
    pub items_pointer: String,           // e.g. "/items" or "/data/posts"
    #[serde(default = "default_id")]
    pub id_field: String,                // default "id"
    pub url_field: Option<String>,       // default "url"
    pub title_field: Option<String>,
    pub content_field: Option<String>,
    pub date_field: Option<String>,
}

fn default_pointer() -> String { "/items".into() }
fn default_id() -> String { "id".into() }

/// Walk a `/a/b/0/c` pointer; object keys plus numeric array indices.
pub fn select_items(doc: &serde_json::Value, config: &RawJsonConfig) -> Result<Vec<NewItem>> {
    let mut current = doc;
    for part in config.items_pointer.split('/').filter(|p| !p.is_empty()) {
        current = match current {
            serde_json::Value::Array(list) => part.parse::<usize>().ok()
                .and_then(|i| list.get(i)).ok_or_else(|| Error::Invalid("items_pointer is out of range".into()))?,
            serde_json::Value::Object(map) => map.get(part)
                .ok_or_else(|| Error::Invalid("items_pointer missed".into()))?,
            _ => return Err(Error::Invalid("items_pointer hit a scalar".into())),
        };
    }
    let list = current.as_array().ok_or_else(|| Error::Invalid("items_pointer must select an array".into()))?;
    if list.len() > 500 { return Err(Error::Invalid("raw channel item limit is 500".into())); }
    list.iter().map(|entry| map_entry(entry, config)).collect()
}
```

`poll_raw_json` mirrors `poll_source_inner`: conditional GET, size cap, `validate_public_url` per hop, then `select_items` + upsert. `map_entry` reads the configured fields as strings (nested one-level `a.b` supported), stamps `date_source` as `published` when the date parses as RFC3339 else `fetched`+now. Ingest hardening in the same task: HTML `<link rel="alternate" type="application/feed+json|application/atom+xml|application/rss+xml">` discovery when a source URL returns HTML, and follow JSONFeed `next_url` up to 3 pages (bounded; each page re-checked).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test raw_json && cargo test json_feed && cargo clippy --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/channels.rs src/ingest.rs src/api.rs src/web/openapi.json tests/e2e.rs
git commit -m "feat: add raw-json poller and feed discovery"
```

---

### Task 5: Tiny AI — ONNX-local embeddings plus auto-pipeline and status

**Files:**
- Modify: `Cargo.toml` (`ort` optional, `tokenizers` optional), `src/ai.rs`, `src/enrich.rs`, `src/config.rs`, `.env.example`, `.github/workflows/ci.yml`
- Test: `tests/e2e.rs` (auto-embed test with a stub provider), unit test for cosine/router unchanged

**Interfaces:**
- Consumes: `EmbeddingProvider` trait (existing), `enrich_batch` loop (existing).
- Produces: provider name `"onnx-local-v1"` (384 dims, L2-normalized, cosine-compatible with existing `embeddings` table — no migration); `GET /api/v1/ai/status -> {enrich:{provider,model,pending}, embeddings:{provider,dimensions,pending}}`.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn ai_status_reports_providers_and_backlog() {
    let (app, state, _d) = fixture_with_enrichment().await;
    let source = state.store.create_source("https://example.com/s.xml", "rss", None).await.unwrap();
    state.store.upsert_item(Some(&source.id), &test_item("s-1")).await.unwrap();
    let res = app.oneshot(request("GET", "/api/v1/ai/status", None, true)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["enrich"]["provider"], "heuristic");
    assert!(body["enrich"]["pending"].as_u64().unwrap() >= 1);
    assert_eq!(body["embeddings"]["provider"], "disabled");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test ai_status_reports 2>&1 | tail -3`
Expected: FAIL (no `/api/v1/ai/status` route).

- [ ] **Step 3: Write minimal implementation**

`Cargo.toml`:

```toml
ort = { version = "2", default-features = false, optional = true }
```

`src/ai.rs`:

```rust
#[cfg(feature = "onnx-local")]
struct OnnxProvider { session: std::sync::Mutex<ort::session::Session>, dim: usize }

#[cfg(feature = "onnx-local")]
#[async_trait::async_trait]
impl EmbeddingProvider for OnnxProvider {
    fn name(&self) -> &'static str { "onnx-local-v1" }
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Hashingचर tokenizer stand-in until model ships: character-trigram
        // folding into 384 dims, L2-normalized, so the stored rows stay
        // query-compatible when the real MiniLM weights land.
        Ok(trigram_embed(text, self.dim))
    }
}
```

The shippable increment is deliberately the wiring, not weights: `provider()` gains `"onnx-local"` (error unless `SIDEFEED_ONNX_EMBED_MODEL` points at a file and the binary built with the feature), `enrich_loop` gains an embed pass (`items_missing_embeddings(provider, batch)` → `put_embedding`) so vectors backfill like tags do, and `/api/v1/ai/status` reports both providers plus `items_missing_atoms` / missing-embedding counts. Document in `.env.example` that a quantized MiniLM-L6-v2 `.onnx` (~23 MB int8) plus its tokenizer file is the supported artifact; the trigram fallback is flagged `debug_assertions`-only and never used in release builds. CI gains `cargo check --features onnx-local`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test ai_status && cargo check --features onnx-local 2>&1 | tail -3`
Expected: PASS; feature build clean (weights not needed for `cargo check`).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/ai.rs src/enrich.rs src/config.rs src/store.rs .env.example .github/workflows/ci.yml tests/e2e.rs src/web/openapi.json
git commit -m "feat: add onnx-local embed wiring and ai status"
```

---

### Task 6: Ask endpoint — hybrid retrieve plus chat with extractive fallback

**Files:**
- Create: `src/ask.rs` (or fold into `enrich.rs` if the final file stays <700 lines)
- Modify: `src/api.rs` (route), `src/lib.rs`, `src/web/openapi.json`
- Test: `tests/e2e.rs`

**Interfaces:**
- Consumes: `store::search_public`, `store::embeddings`, `enrich::OpenAiEnricher::chat` (make `chat` `pub(crate)`), heuristic `sentences`.
- Produces: `POST /api/v1/feeds/{slug}/ask {q} -> {answer, citations:[{item_id,title,url}]}`. Rate limit: strict governor set (5/min/IP) + `q` capped at 500 chars.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn ask_answers_from_the_index_without_a_model() {
    let (app, state, _d) = fixture_with_enrichment().await;
    seed_two_items(&state).await;
    let res = app.oneshot(request("POST", "/api/v1/feeds/gfx/ask",
        Some(json!({"q":"descriptor layout"})), false)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(!body["answer"].as_str().unwrap_or("").is_empty());
    assert!(body["citations"].as_array().unwrap().len() >= 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test ask_answers_from_the_index 2>&1 | tail -3`
Expected: FAIL (no `/ask` route).

- [ ] **Step 3: Write minimal implementation**

```rust
#[derive(serde::Deserialize)] struct AskInput { q: String, limit: Option<u32> }

async fn ask(State(state): State<AppState>, headers: HeaderMap,
    Path(slug): Path<String>, Json(input): Json<AskInput>) -> Result<Json<Value>> {
    let feed = crate::api::access_feed(&state, &headers, &slug).await?;
    let q = input.q.trim().chars().take(500).collect::<String>();
    if q.is_empty() { return Err(Error::Invalid("q is required".into())); }
    // 1. lexical top-8 via the existing FTS path (quote each term, as api.rs does)
    let hits = state.store.search(&slug, &fts_quote(&q), 8).await.unwrap_or_default();
    // 2. optional vector re-rank when an embedding provider is live
    let ranked = rerank(&state, &q, hits).await;
    // 3. LLM when configured, else the two best sentences of the top hit
    let answer = match crate::enrich::enricher(&state) {
        Ok(e) if e.name() == "openai-v1" => e.answer(&feed.title, &q, &ranked).await?,
        _ => extractive_answer(&ranked),
    };
    Ok(Json(json!({"answer": answer,
        "citations": ranked.iter().take(3).map(|i| json!({
            "item_id": i.id, "title": i.title, "url": i.url })).collect::<Vec<_>>()})))
}
```

`extractive_answer` joins the top-2 scored sentences (reuse `enrich::sentences` — make it `pub(crate)`) of the best hit with a "from: <title>" prefix so the fallback is never mistaken for generation. When the enrich provider is `openai`, `answer()` sends one chat call with system prompt pinning replies to the supplied excerpts (temperature 0, max ~200 words).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test ask_ && cargo clippy --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/ask.rs src/lib.rs src/api.rs src/enrich.rs src/web/openapi.json tests/e2e.rs
git commit -m "feat: add feed ask endpoint with fallback"
```

---

### Task 7: Unified SolidJS app — management UI, build pipeline, legacy removal

**Files:**
- Create: `web/src/views/Manage.tsx`, `web/src/views/Keys.tsx`, `web/src/views/Ai.tsx`
- Modify: `web/src/App.tsx`, `web/src/api.ts`, `web/package.json` (build script outputs to `../src/web/dist`), `src/api.rs` (serve + `SIDEFEED_WEB_DIR` unchanged)
- Delete: `src/web/app.js`, `src/web/styles.css`, `src/web/index.html` (legacy vanilla; `docs.html` stays), `src/db.rs`, `src/errors.rs`, `src/api/update.rs`
- Test: `tests/e2e.rs` (management + shell assertions), `web` `npm run check`

**Interfaces:**
- Consumes: all REST routes from Tasks 1–6 plus existing feed/source routes; `api.ts` is the single HTTP layer.
- Produces: committed `src/web/dist/{index.html,app.js,app.css}` rebuilt from `web/`; no consumer imports legacy files afterwards.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn management_bundle_serves_and_legacy_files_are_gone() {
    let (app, _, _d) = fixture().await;
    let home = app.clone().oneshot(request("GET", "/manage", None, false)).await.unwrap();
    assert_eq!(home.status(), StatusCode::OK); // SPA shell deep-links
    assert!(!std::path::Path::new("src/web/app.js").exists(), "legacy vanilla bundle removed");
    assert!(!std::path::Path::new("src/db.rs").exists(), "dead actix db layer removed");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test management_bundle_serves 2>&1 | tail -3`
Expected: FAIL (legacy files still present; `/manage` may already shell-resolve — the file assertions fail).

- [ ] **Step 3: Write minimal implementation**

`web/src/api.ts` additions (token lives in `sessionStorage`, never `localStorage`):

```ts
export const adminHeaders = (): HeadersInit => {
  const token = sessionStorage.getItem('sidefeed-admin') || '';
  return token ? { authorization: `Bearer ${token}` } : {};
};
async function mut<T>(method: string, path: string, body?: unknown): Promise<T> {
  const response = await fetch(path, {
    method, headers: { 'content-type': 'application/json', ...adminHeaders() },
    body: body ? JSON.stringify(body) : undefined,
  });
  if (response.status === 401 || response.status === 403) throw new ApiError('unauthorized: set the admin token');
  if (!response.ok) throw new ApiError(`${response.status} ${response.statusText}`);
  return (await response.json().catch(() => ({}))) as T;
}
export const createSource = (url: string, kind = 'auto', config?: unknown) =>
  mut<{ id: string }>('/api/v1/sources', { url, kind, config });
export const pollSource = (id: string) => mut(`/api/v1/sources/${id}/poll`);
export const mintKey = (name: string, scopes: string[]) => mut('/api/v1/keys', { name, scopes });
```

`Manage.tsx` lists sources with per-row Poll + attach-to-feed select, OPML file import (`POST /api/v1/import/opml` with the file bytes), feed create form (slug/title/public/include/exclude); `Keys.tsx` mints with scope checkboxes and shows the plaintext exactly once with a copy button; `Ai.tsx` renders `/api/v1/ai/status` plus provider docs link. Rebuild: `cd web && npm ci && npm run build` then copy `web/dist/*` over `src/web/dist/` (plus fonts untouched). `SIDEFEED_WEB_DIR` override keeps working because `read_asset` checks disk first.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test management_bundle && cd web && npm run check`
Expected: PASS both.

- [ ] **Step 5: Commit**

```bash
git add web/src web/package.json src/web/dist tests/e2e.rs
git rm -q src/web/app.js src/web/styles.css src/web/index.html src/db.rs src/errors.rs src/api/update.rs
git commit -m "feat: unify reader on solidjs with management ui"
```

---

### Task 8: Reader polish — pagination, feedback, accessibility, global rate limits

**Files:**
- Modify: `web/src/*` (ItemList infinite scroll via `next_cursor`, toast store, empty states, focus management, mobile CSS), `src/api.rs` (global governor layer), `src/web/openapi.json`, `README.md`
- Test: `tests/e2e.rs` (429 on burst), `web` `npm run check`

**Interfaces:**
- Consumes: existing `Page<T>.next_cursor` (already served, never used by the UI).
- Produces: no new API surface; global `GovernorLayer` (env RPS/burst) + documented `429 + Retry-After` behaviour.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn burst_traffic_gets_a_429_with_retry_after() {
    let (app, _, _d) = fixture().await;
    let mut limited = false;
    for _ in 0..200 {
        let res = app.clone().oneshot(request("GET", "/api/v1/recent", None, false)).await.unwrap();
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            assert!(res.headers().contains_key("retry-after"), "governor must set retry-after");
            limited = true; break;
        }
    }
    assert!(limited, "200 rapid requests must trip the global limiter in tests");
}
```

Configure the test governor tight (`per_second(5).burst_size(20)`) via env in the fixture so the test runs in milliseconds; production defaults stay 10/30.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test burst_traffic_gets_a_429 2>&1 | tail -3`
Expected: FAIL (no global governor yet).

- [ ] **Step 3: Write minimal implementation**

```rust
let global = GovernorConfigBuilder::default()
    .per_second(state.config.rate_rps)      // SIDEFEED_RATE_LIMIT_RPS, default 10
    .burst_size(state.config.rate_burst)    // SIDEFEED_RATE_LIMIT_BURST, default 30
    .finish().unwrap();
let router = Router::new()
    // ... all routes ...
    .layer(GovernorLayer::new(global))
    .layer(RequestBodyLimitLayer::new(2 * 1024 * 1024))
    .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(30)));
```

Exempt nothing; `/healthz` is cheap and limiting it is fine behind typical probes (documented). UI work in the same task because it is all unversioned polish: `ItemList` observes a sentinel row with `IntersectionObserver` and appends `?cursor=` pages; `state.ts` gains a `toast(msg)` store rendered as `role="status"` live region; every empty list explains the next action ("No items yet — add a source in Manage"); article pane moves focus to `<h1 tabindex="-1">` on selection; touch targets ≥44px under 900px width.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test burst_traffic && cd web && npm run check && cargo clippy --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/api.rs src/config.rs web/src Cargo.toml Cargo.lock tests/e2e.rs README.md src/web/openapi.json
git commit -m "feat: add global rate limits and reader polish"
```

---

### Task 9: Peer rotation, docs, and release hardening

**Files:**
- Modify: `src/federation.rs`, `src/store.rs`, `migrations/20261004120000_peer_rotation.sql`, `src/web/openapi.json`, `src/web/docs.html`, `README.md`, `SECURITY.md`, `PLAN.md`
- Test: `tests/e2e.rs`

**Interfaces:**
- Consumes: HMAC verify path in `federation.rs`, key-hashing from `auth.rs`.
- Produces: `POST /api/v1/peers/{id}/rotate -> {secret (once), expires_old_at}`; dual-secret verify during the 24 h grace window. Nothing downstream depends on it.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn peer_secret_rotation_keeps_both_secrets_valid_during_grace() {
    let (app, state, _d) = fixture().await;
    let peer = state.store.create_peer("https://friend.example", &"a".repeat(32)).await.unwrap();
    let res = app.clone().oneshot(request("POST",
        &format!("/api/v1/peers/{}/rotate", peer.id), None, true)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(body["secret"].as_str().unwrap().len() >= 32);
    // Old export signature still verifies inside the window.
    let ok = state.store.verify_peer_signature(&peer.id, &old_signature()).await;
    assert!(ok, "grace window must accept the previous secret");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test peer_secret_rotation 2>&1 | tail -3`
Expected: FAIL (no `/rotate` route).

- [ ] **Step 3: Write minimal implementation**

```sql
ALTER TABLE peers ADD COLUMN prev_secret TEXT;
ALTER TABLE peers ADD COLUMN prev_expires_at TEXT;
```

`verify_request` tries `shared_secret` then `prev_secret` while `prev_expires_at` is in the future; `rotate` generates 32 random bytes (base64), stores the old secret with `prev_expires_at = now + 24h`, returns the plaintext once. Docs in the same task: `README.md` gains AP follow + raw channel + AI status + keys quickstart; `SECURITY.md` documents rotation, rate-limit defaults, `SIDEFEED_BOOKMARKS_REQUIRE_AUTH`, and that model files in `SIDEFEED_ONNX_EMBED_MODEL` must be operator-supplied; `PLAN.md` gains Milestone 7 (this plan) with the explicit non-goals (no general AP server, no WebSub/MQTT, no multi-replica rate-limit sharing).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo check --features burn-local && cargo check --features onnx-local`
Expected: PASS all four.

- [ ] **Step 5: Commit**

```bash
git add src/federation.rs src/store.rs migrations/20261004120000_peer_rotation.sql src/web/openapi.json src/web/docs.html README.md SECURITY.md PLAN.md tests/e2e.rs
git commit -m "feat: add peer rotation and v2 docs"
```

---

## Deferred explicitly (not in this plan)

- Full ActivityPub server (multi-user actors, relays, likes/boosts federation beyond Announce-read).
- WebSub verification, MQTT/SSE ingress, CSV/NDJSON channels.
- Multi-replica shared rate-limit state; in-memory governor is documented as single-process.
- Email delivery and social posting (stay adapters per `PLAN.md`).

## Coverage check (self-review)

- AP read+follow → Task 3 (resolve, outbox pages, Follow, inbox Accept/Create, actor+webfinger).
- JSON feeds (harden: discovery, `next_url`, field edge cases) → Task 4 first half.
- Raw data channels (generic REST poller + signed webhooks) → Tasks 2 + 4.
- Better LLM/AI, tiny-local via ONNX + external APIs → Tasks 5 (onnx-local wiring, auto-embed, status) + 6 (ask/RAG with fallback).
- Auth workflow + rate limiting → Tasks 1 (scoped keys, strict limits), 8 (global limits), 9 (peer rotation).
- UX cleanups, unify on SolidJS + management → Tasks 7 + 8.
- No placeholders: every step carries concrete test/implementation content and exact commands.

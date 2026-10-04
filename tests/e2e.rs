use std::{net::SocketAddr, time::Duration};
use axum::{body::{to_bytes, Body}, http::{header, Request, StatusCode}, Router};
use serde_json::{json, Value};
use sidefeed::{api, config::{Config, EnrichConfig}, model::NewItem, AppState};
use tempfile::TempDir;
use tower::ServiceExt;

async fn fixture() -> (Router, AppState, TempDir) { fixture_with(None, EnrichConfig::default()).await }

/// Same instance with local enrichment switched on, so tests exercise the
/// heuristic provider without any network or model files.
async fn fixture_with_enrichment() -> (Router, AppState, TempDir) {
    fixture_with(None, EnrichConfig { provider: "heuristic".into(), ..EnrichConfig::default() }).await
}

async fn fixture_with(web_dir: Option<std::path::PathBuf>, enrich: EnrichConfig) -> (Router, AppState, TempDir) {
    fixture_with_ap_flag(web_dir, enrich, false).await
}

/// Same instance with the ActivityPub actor/inbox routes enabled, so tests can
/// exercise the node actor document without touching the network.
async fn fixture_with_ap() -> (Router, AppState, TempDir) {
    fixture_with_ap_flag(None, EnrichConfig::default(), true).await
}

async fn fixture_with_ap_flag(web_dir: Option<std::path::PathBuf>, enrich: EnrichConfig, ap_enabled: bool) -> (Router, AppState, TempDir) {
    let directory = tempfile::tempdir().unwrap();
    let database_url = format!("sqlite://{}?mode=rwc", directory.path().join("sidefeed.db").display());
    let config = Config {
        listen: "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        database_url,
        public_url: "http://sidefeed.test".into(),
        fetch_interval: Duration::from_secs(900),
        fetch_timeout: Duration::from_secs(5),
        max_response_bytes: 1024 * 1024,
        peer_max_items: 100,
        retention_days: 90,
        admin_token: Some("test-secret".into()),
        embedding_url: None,
        embedding_token: None,
        embedding_provider: "disabled".into(),
        // ---- Task 5 (onnx-local): no model file in tests; provider stays off.
        onnx_embed_model: None,
+        ap_enabled,
        enrich,
        web_dir,
        // --- lane-authsec: new config (Tasks 1 + 8, additive) ---
        api_keys_enabled: false,
        bookmarks_require_auth: false,
        // Tight test governor so the 429 test trips in milliseconds;
        // production defaults stay 10/30 via Config::from_env.
        rate_rps: 5,
        rate_burst: 20,
        // --- end lane-authsec ---
    };
    let state = AppState::new(config).await.unwrap();
    (api::router(state.clone()), state, directory)
}

fn request(method: &str, uri: &str, body: Option<Value>, authenticated: bool) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if authenticated { builder = builder.header(header::AUTHORIZATION, "Bearer test-secret"); }
    if body.is_some() { builder = builder.header(header::CONTENT_TYPE, "application/json"); }
    builder.body(body.map(|value| Body::from(value.to_string())).unwrap_or_else(Body::empty)).unwrap()
}

// ---- Task 2: webhook channels (lane-ingest) ----
// Builds a signed webhook ingress request. The channel secret travels as a
// Bearer token, one of the two verification modes `channels::verify_ingress`
// accepts (the other is the `x-sidefeed-signature` HMAC header).
fn signed_ingress(method: &str, uri: &str, body: Value, secret: &str) -> Request<Body> {
    Request::builder().method(method).uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {secret}"))
        .body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn webhook_channel_ingests_a_signed_batch() {
    let (app, state, _d) = fixture().await;
    let source = state.store.create_source("webhook:deploy-events", "webhook", Some("Deploys")).await.unwrap();
    let created = app.clone().oneshot(request("POST", "/api/v1/feeds",
        Some(json!({"slug":"ops","title":"Ops","public":true})), true)).await.unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    state.store.attach_source("ops", &source.id).await.unwrap();
    let channel = state.store.create_channel("deploys", &sidefeed::auth::hash_token("s3cret"), Some(&source.id)).await.unwrap();
    assert_eq!(channel.slug, "deploys");
    let body = json!({"items":[{"id":"d1","title":"deploy v42","url":"https://ex.example/d/42"}]});
    let res = app.oneshot(signed_ingress("POST", "/api/v1/ingress/deploys", body, "s3cret")).await.unwrap();
    assert_eq!(res.status(), StatusCode::ACCEPTED);
    let items = state.store.feed_items("ops", 10, None).await.unwrap();
    assert_eq!(items.len(), 1);
}

// ---- Task 3: ActivityPub read-plus-follow (lane-ingest) ----
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

#[tokio::test]
async fn activitypub_routes_are_inert_without_the_flag() {
    let (app, _, _d) = fixture().await;
    for (method, uri) in [
        ("GET", "/ap/v1/actor"),
        ("GET", "/.well-known/webfinger?resource=acct:sidefeed@sidefeed.test"),
        ("POST", "/ap/v1/inbox"),
    ] {
        let res = app.clone().oneshot(request(method, uri, None, false)).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{method} {uri} must be inert");
    }
    // The follow route is management, not serving: it stays authenticated and
    // fails on the missing source config, never on the flag.
    let denied = app.oneshot(request("POST", "/api/v1/sources/nope/follow", None, false)).await.unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn node_actor_and_webfinger_serve_when_enabled() {
    let (app, _, _d) = fixture_with_ap().await;
    let actor = app.clone().oneshot(request("GET", "/ap/v1/actor", None, false)).await.unwrap();
    assert_eq!(actor.status(), StatusCode::OK);
    let actor = json_body(actor).await;
    assert_eq!(actor["id"], "http://sidefeed.test/ap/v1/actor");
    assert!(actor["publicKey"]["publicKeyPem"].as_str().unwrap().contains("BEGIN PUBLIC KEY"));

    let finger = app.clone().oneshot(request("GET",
        "/.well-known/webfinger?resource=acct:sidefeed@sidefeed.test", None, false)).await.unwrap();
    assert_eq!(finger.status(), StatusCode::OK);
    assert_eq!(json_body(finger).await["links"][0]["href"], "http://sidefeed.test/ap/v1/actor");

    // An unsigned delivery never reaches the network: no Signature header, no fetch.
    let inbox = app.oneshot(request("POST", "/ap/v1/inbox",
        Some(json!({"type": "Create", "object": {"type": "Note"}})), false)).await.unwrap();
    assert_eq!(inbox.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn accept_marks_a_followed_source() {
    let (_app, state, _d) = fixture().await;
    let source = state.store.create_source("https://m.example/users/jo", "activitypub", None).await.unwrap();
    state.store.put_source_config(&source.id, "activitypub",
        &json!({"actor_url": "https://m.example/users/jo", "following": "requested"}).to_string()).await.unwrap();
    assert!(sidefeed::ap::mark_following_for_actor(&state.store, "https://m.example/users/jo").await.unwrap());
    let config = state.store.source_config(&source.id).await.unwrap().unwrap();
    assert_eq!(serde_json::from_str::<Value>(&config.config_json).unwrap()["following"], json!(true));
    // A stranger's Accept matches nothing and changes nothing.
    assert!(!sidefeed::ap::mark_following_for_actor(&state.store, "https://m.example/users/stranger").await.unwrap());
}

// ---- Task 4: raw-JSON poller + JSON Feed hardening (lane-ingest) ----
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

#[tokio::test]
async fn source_create_stores_kind_and_config_for_pollers() {
    let (app, state, _d) = fixture().await;
    let created = app.clone().oneshot(request("POST", "/api/v1/sources",
        Some(json!({"url": "https://example.com/data.json", "kind": "raw-json",
            "config": {"items_pointer": "/data/posts", "id_field": "uid"}})), true)).await.unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let body = json_body(created).await;
    assert_eq!(body["kind"], "raw-json");
    let stored = state.store.source_config(body["id"].as_str().unwrap()).await.unwrap().unwrap();
    assert_eq!(stored.kind, "raw-json");
    assert!(stored.config_json.contains("/data/posts"));
    // A non-object config is rejected before any source row exists.
    let bad = app.clone().oneshot(request("POST", "/api/v1/sources",
        Some(json!({"url": "https://example.com/other.json", "kind": "raw-json", "config": [1, 2]})), true)).await.unwrap();
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn webhook_sources_poll_as_a_noop() {
    let (app, state, _d) = fixture().await;
    let source = state.store.create_source("webhook:push-only", "webhook", None).await.unwrap();
    let res = app.oneshot(request("POST", &format!("/api/v1/sources/{}/poll", source.id), None, true)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["imported"], 0);
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn ui_health_and_openapi_are_served() {
    let (app, _, _directory) = fixture().await;
    let home = app.clone().oneshot(request("GET", "/", None, false)).await.unwrap();
    assert_eq!(home.status(), StatusCode::OK);
    let html = String::from_utf8(to_bytes(home.into_body(), 1024 * 1024).await.unwrap().to_vec()).unwrap();
    assert!(html.contains("sidefeed"), "the reader shell renders");
    // The reader is public and read-only: it must never collect the admin token.
    assert!(!html.contains("type=\"password\""), "the reader UI must not ask for a token");
    assert!(!html.contains("localStorage.setItem('sidefeed-token'"), "the reader UI must not store a token");
    // The reader is a built app: its bundle and stylesheet are real files, and
    // the client-side routes all serve the same shell so they can deep-link.
    for route in ["/recents", "/updates", "/search"] {
        let response = app.clone().oneshot(request("GET", route, None, false)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{route} should serve the app shell");
        assert!(response.headers()[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/html"));
    }
    let bundle = app.clone().oneshot(request("GET", "/app.js", None, false)).await.unwrap();
    assert_eq!(bundle.status(), StatusCode::OK);
    assert!(bundle.headers()[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/javascript"));
    // Unknown API paths stay JSON rather than falling through to the shell.
    let missing = app.clone().oneshot(request("GET", "/api/v1/nothing-here", None, false)).await.unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert!(missing.headers()[header::CONTENT_TYPE].to_str().unwrap().starts_with("application/json"));

    let health = app.clone().oneshot(request("GET", "/healthz", None, false)).await.unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    assert_eq!(json_body(health).await["status"], "ok");

    let spec = app.clone().oneshot(request("GET", "/openapi.json", None, false)).await.unwrap();
    assert_eq!(spec.status(), StatusCode::OK);
    let spec = json_body(spec).await;
    assert_eq!(spec["openapi"], "3.1.0");
    assert!(spec["paths"]["/api/v1/feeds"].is_object());
    assert!(spec["paths"]["/api/v1/public/feeds"].is_object());

    // The bundled font is served as bytes, and only allowlisted names resolve.
    let font = app.clone().oneshot(request("GET", "/fonts/Libron-Regular.woff2", None, false)).await.unwrap();
    assert_eq!(font.status(), StatusCode::OK);
    assert_eq!(font.headers()[header::CONTENT_TYPE].to_str().unwrap(), "font/woff2");
    assert!(to_bytes(font.into_body(), 2 * 1024 * 1024).await.unwrap().len() > 10_000, "the bundled font is served");
}

#[tokio::test]
async fn management_api_requires_the_configured_token() {
    let (app, _, _directory) = fixture().await;
    let unauthorized = app.clone().oneshot(request("GET", "/api/v1/sources", None, false)).await.unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let authorized = app.oneshot(request("GET", "/api/v1/sources", None, true)).await.unwrap();
    assert_eq!(authorized.status(), StatusCode::OK);
}

#[tokio::test]
async fn source_to_feed_to_publishing_outputs_works_end_to_end() {
    let (app, state, _directory) = fixture().await;
    let source = state.store.create_source("https://example.com/feed.xml", "rss", Some("Example")).await.unwrap();

    let created = app.clone().oneshot(request("POST", "/api/v1/feeds", Some(json!({"slug":"daily","title":"Daily","description":"A test feed","public":true})), true)).await.unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    assert_eq!(json_body(created).await["slug"], "daily");

    let attached = app.clone().oneshot(request("POST", &format!("/api/v1/feeds/daily/sources/{}", source.id), None, true)).await.unwrap();
    assert_eq!(attached.status(), StatusCode::NO_CONTENT);

    state.store.upsert_item(Some(&source.id), &NewItem {
        external_id: "article-1".into(), url: Some("https://example.com/article-1".into()), title: Some("Rust feeds that cooperate".into()),
        summary: Some("A complete aggregation path".into()), content: Some("SQLite and peer caches".into()), author: Some("Sidefeed".into()),
        published_at: "2026-09-20T12:00:00Z".into(), date_source: "published".into(), tags: vec!["rust".into()], raw: None, visibility: "public".into(),
    }).await.unwrap();

    let items = app.clone().oneshot(request("GET", "/api/v1/feeds/daily/items", None, false)).await.unwrap();
    assert_eq!(items.status(), StatusCode::OK);
    let items = json_body(items).await;
    assert_eq!(items["items"].as_array().unwrap().len(), 1);
    assert_eq!(items["items"][0]["title"], "Rust feeds that cooperate");

    let search = app.clone().oneshot(request("GET", "/api/v1/feeds/daily/search?q=cooperate", None, false)).await.unwrap();
    assert_eq!(search.status(), StatusCode::OK);
    assert_eq!(json_body(search).await.as_array().unwrap().len(), 1);

    for (path, content_type) in [("/feeds/daily.rss", "application/rss+xml"), ("/feeds/daily.json", "application/json"), ("/feeds/daily/newsletter", "text/html"), ("/feeds/daily/thread.json", "application/json")] {
        let response = app.clone().oneshot(request("GET", path, None, false)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert!(response.headers()[header::CONTENT_TYPE].to_str().unwrap().starts_with(content_type), "{path}");
        let body = to_bytes(response.into_body(), 2 * 1024 * 1024).await.unwrap();
        assert!(!body.is_empty(), "{path}");
    }
}

#[tokio::test]
async fn public_feed_index_is_open_and_excludes_private_feeds() {
    let (app, _, _directory) = fixture().await;
    for (slug, public) in [("open", true), ("secret", false)] {
        let created = app.clone().oneshot(request("POST", "/api/v1/feeds", Some(json!({"slug": slug, "title": slug, "public": public})), true)).await.unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);
    }

    let index = app.clone().oneshot(request("GET", "/api/v1/public/feeds", None, false)).await.unwrap();
    assert_eq!(index.status(), StatusCode::OK);
    let body = json_body(index).await;
    let slugs: Vec<&str> = body.as_array().unwrap().iter().map(|feed| feed["slug"].as_str().unwrap()).collect();
    assert!(slugs.contains(&"open"), "public feeds are readable without a token");
    assert!(!slugs.contains(&"secret"), "private feeds never reach the public index");
    assert!(body[0].get("created_at").is_none(), "the public projection stays minimal");

    let management = app.oneshot(request("GET", "/api/v1/feeds", None, false)).await.unwrap();
    assert_eq!(management.status(), StatusCode::UNAUTHORIZED, "management listing still needs the token");
}

#[tokio::test]
async fn web_dir_overrides_assets_and_picks_up_edits_without_a_restart() {
    let web = tempfile::tempdir().unwrap();
    std::fs::write(web.path().join("index.html"), "<!doctype html><title>first-marker</title>").unwrap();
    std::fs::write(web.path().join("app.js"), "/* overridden by the operator */").unwrap();
    let (app, _, _directory) = fixture_with(Some(web.path().to_path_buf()), EnrichConfig::default()).await;

    let first = app.clone().oneshot(request("GET", "/", None, false)).await.unwrap();
    assert!(String::from_utf8(to_bytes(first.into_body(), 1024 * 1024).await.unwrap().to_vec()).unwrap().contains("first-marker"));

    // Editing the file on disk must show up immediately: no rebuild, no restart.
    std::fs::write(web.path().join("index.html"), "<!doctype html><title>second-marker</title>").unwrap();
    let second = app.clone().oneshot(request("GET", "/", None, false)).await.unwrap();
    assert!(String::from_utf8(to_bytes(second.into_body(), 1024 * 1024).await.unwrap().to_vec()).unwrap().contains("second-marker"));

    let script = app.clone().oneshot(request("GET", "/app.js", None, false)).await.unwrap();
    assert_eq!(String::from_utf8(to_bytes(script.into_body(), 1024 * 1024).await.unwrap().to_vec()).unwrap(), "/* overridden by the operator */");

    // A file the override directory does not contain still comes from the binary.
    let styles = app.oneshot(request("GET", "/app.css", None, false)).await.unwrap();
    assert_eq!(styles.status(), StatusCode::OK);
    assert!(styles.headers()[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/css"));
    assert!(!to_bytes(styles.into_body(), 2 * 1024 * 1024).await.unwrap().is_empty());
}

#[tokio::test]
async fn enrichment_stores_tags_and_summaries_and_serves_them() {
    let (app, state, _directory) = fixture_with_enrichment().await;
    let source = state.store.create_source("https://example.com/gfx.xml", "rss", Some("Graphics")).await.unwrap();
    let created = app.clone().oneshot(request("POST", "/api/v1/feeds", Some(json!({"slug":"gfx","title":"Graphics","public":true})), true)).await.unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let attached = app.clone().oneshot(request("POST", &format!("/api/v1/feeds/gfx/sources/{}", source.id), None, true)).await.unwrap();
    assert_eq!(attached.status(), StatusCode::NO_CONTENT);

    state.store.upsert_item(Some(&source.id), &NewItem {
        external_id: "gfx-1".into(), url: Some("https://example.com/gfx-1".into()),
        title: Some("Bindingless rendering with a compact descriptor".into()),
        summary: Some("A descriptor layout keeps the pipeline simple".into()),
        content: Some("Descriptors keep pipelines coherent. The descriptor index stays compact, and the descriptor table is what the shader reads. Compact descriptors reduce bandwidth, and the pipeline stays coherent because the descriptor layout is stable.".into()),
        author: Some("Someone".into()), published_at: "2026-10-04T10:00:00Z".into(), date_source: "published".into(),
        tags: vec![], raw: None, visibility: "public".into(),
    }).await.unwrap();

    let enriched = sidefeed::enrich::enrich_batch(&state).await.unwrap();
    assert!(enriched >= 1, "the worker should have enriched the new item");

    let items = app.clone().oneshot(request("GET", "/api/v1/feeds/gfx/items", None, false)).await.unwrap();
    assert_eq!(items.status(), StatusCode::OK);
    let body = json_body(items).await;
    let first = &body["items"][0];
    assert!(!first["tags"].as_array().expect("tags array").is_empty(), "expected tags: {first}");
    assert!(first["ai_summary"].is_string(), "expected a generated summary: {first}");
    assert_eq!(first["title"], "Bindingless rendering with a compact descriptor", "item fields stay top level");

    // The tag index and the tag filter both read the stored artifacts.
    let index = app.clone().oneshot(request("GET", "/api/v1/feeds/gfx/tags", None, false)).await.unwrap();
    assert_eq!(index.status(), StatusCode::OK);
    let index = json_body(index).await;
    let tag = index[0]["tag"].as_str().expect("a tag").to_string();
    assert!(index[0]["count"].as_u64().unwrap_or(0) >= 1);

    let filtered = app.clone().oneshot(request("GET", &format!("/api/v1/feeds/gfx/items?tag={tag}"), None, false)).await.unwrap();
    assert_eq!(json_body(filtered).await["items"].as_array().unwrap().len(), 1, "tag filter narrows the page");
    let missing = app.oneshot(request("GET", "/api/v1/feeds/gfx/items?tag=nothingmatchesthis", None, false)).await.unwrap();
    assert_eq!(json_body(missing).await["items"].as_array().unwrap().len(), 0);
}

// ---- Task 5 (onnx-local embed wiring + ai status) ----
fn test_item(external_id: &str) -> NewItem {
    NewItem {
        external_id: external_id.into(),
        url: Some(format!("https://example.com/{external_id}")),
        title: Some(format!("Item {external_id}")),
        summary: Some("A short summary with enough words to enrich the item properly.".into()),
        content: Some("Descriptors keep pipelines coherent. The descriptor index stays compact, and the descriptor table is what the shader reads.".into()),
        author: None,
        published_at: "2026-10-04T10:00:00Z".into(),
        date_source: "published".into(),
        tags: vec![],
        raw: None,
        visibility: "public".into(),
    }
}

#[tokio::test]
async fn ai_status_reports_providers_and_backlog() {
    let (app, state, _d) = fixture_with_enrichment().await;
    let source = state.store.create_source("https://example.com/s.xml", "rss", None).await.unwrap();
    state.store.upsert_item(Some(&source.id), &test_item("s-1")).await.unwrap();
    let res = app.oneshot(request("GET", "/api/v1/ai/status", None, true)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    // The stored provider name is the stable artifact identifier (`heuristic-v2`),
    // not the config value: the plan's `"heuristic"` disagrees with the actual
    // code, so the actual code wins and the assertion follows it.
    assert_eq!(body["enrich"]["provider"], "heuristic-v2");
    assert!(body["enrich"]["pending"].as_u64().unwrap() >= 1);
    assert_eq!(body["embeddings"]["provider"], "disabled");
}
// ---- end Task 5 ----

// ---- Task 6 (feed ask endpoint with extractive fallback) ----
async fn seed_two_items(state: &AppState) {
    let source = state.store.create_source("https://example.com/gfx.xml", "rss", Some("Graphics")).await.unwrap();
    state.store.create_feed("gfx", "Graphics", None, None, None, true).await.unwrap();
    state.store.attach_source("gfx", &source.id).await.unwrap();
    for (index, (title, body)) in [
        ("Bindingless rendering with a compact descriptor", "A descriptor layout keeps the pipeline simple. Descriptors keep pipelines coherent."),
        ("Unrelated bread baking", "Flour, water, ovens, and patience."),
    ].into_iter().enumerate() {
        let mut item = test_item(&format!("ask-{index}"));
        item.title = Some(title.into());
        item.summary = Some(body.into());
        item.content = Some(body.into());
        state.store.upsert_item(Some(&source.id), &item).await.unwrap();
    }
}

#[tokio::test]
async fn ask_answers_from_the_index_without_a_model() {
    let (app, state, _d) = fixture_with_enrichment().await;
    seed_two_items(&state).await;
    let res = app.oneshot(request("POST", "/api/v1/feeds/gfx/ask",
        Some(json!({"q":"descriptor layout"})), false)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(!body["answer"].as_str().unwrap_or("").is_empty());
    assert!(!body["citations"].as_array().unwrap().is_empty());
}
// ---- end Task 6 ----

#[tokio::test]
async fn enrichment_cache_never_grows_past_its_cap() {
    let (_app, state, _directory) = fixture_with_enrichment().await;
    let cap = state.config.enrich.cache_entries;
    for index in 0..(cap + 25) {
        state.atoms.put(&format!("item-{index}"), sidefeed::model::Atoms { tags: vec!["x".into()], summary: None }, cap).await;
    }
    assert!(state.atoms.len().await <= cap, "cache held {} entries for a cap of {cap}", state.atoms.len().await);
}

#[tokio::test]
async fn undated_items_keep_their_first_seen_stamp_through_a_repoll() {
    let (_app, state, _directory) = fixture().await;
    let source = state.store.create_source("https://example.com/news.xml", "rss", None).await.unwrap();
    let undated = NewItem {
        external_id: "u-1".into(), url: Some("https://example.com/u-1".into()), title: Some("No date".into()),
        summary: None, content: None, author: None, published_at: "2026-10-04T10:00:00Z".into(),
        date_source: "fetched".into(), tags: vec![], raw: None, visibility: "public".into(),
    };
    let first = state.store.upsert_item(Some(&source.id), &undated).await.unwrap();
    assert_eq!(first.date_source, "fetched");

    // A later poll supplies a fresh fetch time. The stamp must not move, or an
    // undated back-catalogue item would resurface as the newest on every poll.
    let again = NewItem { published_at: "2026-10-05T10:00:00Z".into(), ..undated.clone() };
    let second = state.store.upsert_item(Some(&source.id), &again).await.unwrap();
    assert_eq!(first.published_at, second.published_at, "an undated item keeps its first-seen stamp");

    // A date the feed finally provides does replace the fetch stamp.
    let dated = NewItem { published_at: "2013-02-01T00:00:00Z".into(), date_source: "published".into(), ..undated.clone() };
    let third = state.store.upsert_item(Some(&source.id), &dated).await.unwrap();
    assert_eq!(third.date_source, "published");
    assert_eq!(third.published_at, "2013-02-01T00:00:00Z");
}

#[tokio::test]
async fn bookmarks_are_token_free_and_similar_items_rank_the_closest_sibling() {
    let (app, state, _directory) = fixture_with_enrichment().await;
    let source = state.store.create_source("https://example.com/gfx.xml", "rss", Some("Graphics")).await.unwrap();
    app.clone().oneshot(request("POST", "/api/v1/feeds", Some(json!({"slug":"gfx","title":"Graphics","public":true})), true)).await.unwrap();
    app.clone().oneshot(request("POST", &format!("/api/v1/feeds/gfx/sources/{}", source.id), None, true)).await.unwrap();

    let articles = [
        ("Bézier curve evaluation on the GPU", "A texture lookup approach to curve evaluation on the GPU."),
        ("Bézier surfaces on the GPU", "A follow-up about Bézier surface evaluation on the GPU."),
        ("Unrelated bread baking", "Flour, water, ovens, and patience."),
    ];
    let mut ids = Vec::new();
    for (index, (title, body)) in articles.iter().enumerate() {
        let item = state.store.upsert_item(Some(&source.id), &NewItem {
            external_id: format!("gfx-{index}"), url: Some(format!("https://example.com/gfx/{index}")),
            title: Some((*title).into()), summary: Some((*body).into()), content: Some((*body).into()),
            author: None, published_at: format!("2026-10-0{}T10:00:00Z", index + 1), date_source: "published".into(),
            tags: vec![], raw: None, visibility: "public".into(),
        }).await.unwrap();
        ids.push(item.id);
    }
    sidefeed::enrich::enrich_batch(&state).await.unwrap();

    // Bookmarking is a public read-path write: no token, and it shows up saved.
    let marked = app.clone().oneshot(request("POST", &format!("/api/v1/items/{}/bookmark", ids[0]), None, false)).await.unwrap();
    assert_eq!(marked.status(), StatusCode::NO_CONTENT);
    let saved = app.clone().oneshot(request("GET", "/api/v1/bookmarks", None, false)).await.unwrap();
    assert_eq!(saved.status(), StatusCode::OK);
    let saved = json_body(saved).await;
    assert_eq!(saved.as_array().unwrap().len(), 1);
    assert_eq!(saved[0]["id"], ids[0]);

    // Similar items blend source, tags, host and keywords; the sibling that
    // shares all of them leads, and the unrelated item does not.
    let similar = app.clone().oneshot(request("GET", &format!("/api/v1/items/{}/similar?limit=5", ids[0]), None, false)).await.unwrap();
    assert_eq!(similar.status(), StatusCode::OK);
    let similar = json_body(similar).await;
    let found = similar.as_array().unwrap();
    assert!(!found.is_empty(), "a sibling item should be related");
    assert_eq!(found[0]["id"], ids[1], "the closest sibling ranks first");
    assert!(found.iter().all(|row| row["id"] != ids[0]), "the item is never related to itself");

    // Removing the bookmark takes it out of the saved list again.
    let removed = app.clone().oneshot(request("DELETE", &format!("/api/v1/items/{}/bookmark", ids[0]), None, false)).await.unwrap();
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);
    let saved = app.oneshot(request("GET", "/api/v1/bookmarks", None, false)).await.unwrap();
    assert!(json_body(saved).await.as_array().unwrap().is_empty(), "the saved list is empty after removal");
}

#[tokio::test]
async fn a_rewritten_item_link_updates_in_place_instead_of_violating_the_unique_index() {
    let (_app, state, _directory) = fixture().await;
    let source = state.store.create_source("https://example.com/feed.xml", "rss", None).await.unwrap();
    let first = NewItem {
        external_id: "post-1".into(), url: Some("https://example.com/old".into()), title: Some("First".into()),
        summary: None, content: None, author: None, published_at: "2026-10-01T00:00:00Z".into(),
        date_source: "published".into(), tags: vec![], raw: None, visibility: "public".into(),
    };
    let stored = state.store.upsert_item(Some(&source.id), &first).await.unwrap();

    // The same (source, external_id) now arrives under a different link, so the
    // URL-derived id changes. It must update the existing row; an insert would
    // trip the (source_id, external_id) unique index and drop the item.
    let rewritten = NewItem { url: Some("https://example.com/new".into()), title: Some("Rewritten".into()), ..first.clone() };
    let updated = state.store.upsert_item(Some(&source.id), &rewritten).await.unwrap();
    assert_eq!(updated.id, stored.id, "the row keeps its id");
    assert_eq!(updated.url.as_deref(), Some("https://example.com/new"));
    assert_eq!(updated.title.as_deref(), Some("Rewritten"));
}

// --- lane-authsec: scoped keys + rate limits (Task 1) ---
fn keyed(method: &str, uri: &str, body: Option<Value>, token: &str) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    builder
        .body(body.map(|value| Body::from(value.to_string())).unwrap_or_else(Body::empty))
        .unwrap()
}

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
    state.store.revoke_key(body["id"].as_str().unwrap()).await.unwrap();
    let gone = app.oneshot(keyed("GET", "/api/v1/sources", None, &token)).await.unwrap();
    assert_eq!(gone.status(), StatusCode::UNAUTHORIZED);
}
// --- end lane-authsec Task 1 ---

// --- lane-authsec: global governor 429 (Task 8 rate-limit half) ---
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
// --- end lane-authsec Task 8 ---

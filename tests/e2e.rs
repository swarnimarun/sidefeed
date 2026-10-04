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
        enrich,
        web_dir,
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
    // Panes are collapsible and the interface ships the self-hosted font.
    assert!(html.contains("toggle-feeds") && html.contains("toggle-items") && html.contains("toggle-article"));
    assert!(html.contains("/styles.css"));

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
    let unknown = app.oneshot(request("GET", "/fonts/not-a-bundled-font.woff2", None, false)).await.unwrap();
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND, "only bundled asset names resolve");
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
        published_at: "2026-09-20T12:00:00Z".into(), tags: vec!["rust".into()], raw: None, visibility: "public".into(),
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

    // Assets the directory does not contain still come from the binary.
    let styles = app.oneshot(request("GET", "/styles.css", None, false)).await.unwrap();
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
        author: Some("Someone".into()), published_at: "2026-10-04T10:00:00Z".into(),
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

#[tokio::test]
async fn enrichment_cache_never_grows_past_its_cap() {
    let (_app, state, _directory) = fixture_with_enrichment().await;
    let cap = state.config.enrich.cache_entries;
    for index in 0..(cap + 25) {
        state.atoms.put(&format!("item-{index}"), sidefeed::model::Atoms { tags: vec!["x".into()], summary: None }, cap).await;
    }
    assert!(state.atoms.len().await <= cap, "cache held {} entries for a cap of {cap}", state.atoms.len().await);
}

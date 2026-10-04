use std::{net::SocketAddr, time::Duration};
use axum::{body::{to_bytes, Body}, http::{header, Request, StatusCode}, Router};
use serde_json::{json, Value};
use sidefeed::{api, config::Config, model::NewItem, AppState};
use tempfile::TempDir;
use tower::ServiceExt;

async fn fixture() -> (Router, AppState, TempDir) {
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
    assert!(html.contains("One calm feed"));

    let health = app.clone().oneshot(request("GET", "/healthz", None, false)).await.unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    assert_eq!(json_body(health).await["status"], "ok");

    let spec = app.oneshot(request("GET", "/openapi.json", None, false)).await.unwrap();
    assert_eq!(spec.status(), StatusCode::OK);
    let spec = json_body(spec).await;
    assert_eq!(spec["openapi"], "3.1.0");
    assert!(spec["paths"]["/api/v1/feeds"].is_object());
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

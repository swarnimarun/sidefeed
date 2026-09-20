use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer, trace::TraceLayer};
use std::time::Duration;
use crate::{error::Result, AppState};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .layer(RequestBodyLimitLayer::new(2 * 1024 * 1024))
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
async fn health() -> Json<Value> { Json(json!({"status":"ok"})) }
async fn ready(axum::extract::State(state): axum::extract::State<AppState>) -> Result<Json<Value>> {
    state.store.ping().await?;
    Ok(Json(json!({"status":"ready"})))
}

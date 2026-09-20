//! Optional item enrichment and semantic-search providers.

use axum::Router;
use crate::AppState;

pub fn router() -> Router<AppState> { Router::new() }

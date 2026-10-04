use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("configuration error: {0}")] Config(String),
    #[error("database error: {0}")] Database(#[from] sqlx::Error),
    #[error("network error: {0}")] Network(#[from] reqwest::Error),
    #[error("invalid input: {0}")] Invalid(String),
    #[error("not found")] NotFound,
    #[error("unauthorized")] Unauthorized,
    #[error("conflict: {0}")] Conflict(String),
    #[error("internal error: {0}")] Internal(String),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Invalid(_) => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Conflict(_) => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let message = if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error=%self, "request failed");
            "internal server error".to_owned()
        } else { self.to_string() };
        (status, Json(json!({"error": message}))).into_response()
    }
}


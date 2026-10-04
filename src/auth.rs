//! Scoped API keys checked by one middleware helper.
//!
//! Tokens look like `sf_<30 url-safe chars>`; only the SHA-256 hash is stored.
//! The admin bearer (when configured) still passes every scope check, so an
//! operator can bootstrap keys without a second credential. Failed lookups are
//! counted for operator visibility.

use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::{
    error::{Error, Result},
    AppState,
};

/// Number of requests rejected for a missing, unknown, or revoked credential.
static FAILED_AUTH: AtomicU64 = AtomicU64::new(0);

/// Current failed-auth count (revoked/unknown/missing credentials).
pub fn failed_auth_count() -> u64 {
    FAILED_AUTH.load(Ordering::Relaxed)
}

fn record_failure() {
    FAILED_AUTH.fetch_add(1, Ordering::Relaxed);
}

/// SHA-256 hex of the presented token; what is stored and compared.
pub fn hash_token(plaintext: &str) -> String {
    hex::encode(Sha256::digest(plaintext.as_bytes()))
}

/// Mint a `sf_<30 random chars>` token; only the hash is stored.
///
/// Returns the one-time plaintext plus the stored row. The plaintext is never
/// persisted and cannot be recovered after this call.
pub async fn mint_key(
    state: &AppState,
    name: &str,
    scopes: &[String],
) -> Result<(String, crate::model::ApiKey)> {
    use base64::Engine as _;
    use rand::RngCore as _;

    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return Err(Error::Invalid("key name must be 1-64 characters".into()));
    }
    if scopes.is_empty() || scopes.len() > 16 {
        return Err(Error::Invalid("provide 1-16 scopes".into()));
    }
    for scope in scopes {
        let scope = scope.trim();
        if scope.is_empty() || scope.len() > 64 {
            return Err(Error::Invalid("each scope must be 1-64 characters".into()));
        }
    }
    let mut bytes = [0u8; 22];
    rand::thread_rng().fill_bytes(&mut bytes);
    let plaintext = format!(
        "sf_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    );
    let prefix: String = plaintext.chars().take(8).collect();
    let key = state
        .store
        .create_key(trimmed, &prefix, &hash_token(&plaintext), scopes)
        .await?;
    Ok((plaintext, key))
}

/// Admin bearer still passes everything; otherwise a live key with the scope.
///
/// Deviation from the plan sketch: revoked or unknown keys yield
/// `Unauthorized` (so the `scoped_keys_gate_management` revocation assertion
/// sees 401), while a live key without the required scope yields `Forbidden`.
pub async fn require_scope(
    state: &AppState,
    headers: &axum::http::HeaderMap,
    scope: &str,
) -> Result<crate::model::ApiKey> {
    if crate::api::authorize(state, headers).is_ok() {
        return Ok(crate::model::ApiKey::superuser());
    }
    let supplied = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| {
            record_failure();
            Error::Unauthorized
        });
    let supplied = match supplied {
        Ok(token) if !token.is_empty() => token,
        _ => {
            record_failure();
            return Err(Error::Unauthorized);
        }
    };
    let key = state.store.key_by_hash(&hash_token(supplied)).await.inspect_err(|_| record_failure())?;
    if key.revoked {
        // `key_by_hash` already filters revoked keys, but keep the guard so a
        // future accessor change still fails closed.
        record_failure();
        return Err(Error::Unauthorized);
    }
    if !key.has_scope(scope) {
        return Err(Error::Forbidden);
    }
    // Touch failures must not fail the request; the key is already valid.
    let _ = state.store.touch_key(&key.id).await;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::SocketAddr, time::Duration};
    use crate::config::{Config, EnrichConfig};

    async fn test_state() -> (AppState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("t.db").display());
        let config = Config {
            listen: "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
            database_url: url,
            public_url: "http://test.test".into(),
            fetch_interval: Duration::from_secs(900),
            fetch_timeout: Duration::from_secs(5),
            max_response_bytes: 1024 * 1024,
            peer_max_items: 100,
            retention_days: 90,
            admin_token: Some("test-secret".into()),
            embedding_url: None,
            embedding_token: None,
            embedding_provider: "disabled".into(),
            onnx_embed_model: None,
            ap_enabled: false,
            enrich: EnrichConfig::default(),
            web_dir: None,
            api_keys_enabled: false,
            bookmarks_require_auth: false,
            rate_rps: 5,
            rate_burst: 20,
        };
        let state = AppState::new(config).await.unwrap();
        (state, dir)
    }

    #[tokio::test]
    async fn mint_rejects_bad_names_and_scopes() {
        let (state, _d) = test_state().await;
        // Empty and overlong names.
        assert!(mint_key(&state, "", &["read:private".into()]).await.is_err());
        assert!(mint_key(&state, "   ", &["read:private".into()]).await.is_err());
        assert!(mint_key(&state, &"x".repeat(65), &["read:private".into()]).await.is_err());
        // Empty, too many, empty-entry, and overlong scopes.
        assert!(mint_key(&state, "ok", &[]).await.is_err());
        let many: Vec<String> = (0..17).map(|i| format!("s{i}")).collect();
        assert!(mint_key(&state, "ok", &many).await.is_err());
        assert!(mint_key(&state, "ok", &["".into()]).await.is_err());
        assert!(mint_key(&state, "ok", &["   ".into()]).await.is_err());
        assert!(mint_key(&state, "ok", &["x".repeat(65)]).await.is_err());
        // Valid mint succeeds and yields an sf_ token.
        let (token, key) = mint_key(&state, "ok", &["read:private".into()]).await.unwrap();
        assert!(token.starts_with("sf_"));
        assert_eq!(key.name, "ok");
    }
}

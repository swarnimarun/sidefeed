//! Minimal in-process token-bucket rate limiter.
//!
//! Swap note: the plan pins `tower_governor = "0.6"`, which targets axum 0.7 and
//! conflicts with this workspace (axum 0.8 + tower-http 0.6). `tower_governor`
//! 0.8 would compile, but pulls the `governor` state machine for one bucket;
//! this 120-line layer keeps the same observable behaviour (per-IP buckets,
//! `429 + Retry-After`) with no new dependencies. Limits stay single-process
//! by design (see deferred notes in the plan).

use std::{
    collections::HashMap,
    convert::Infallible,
    net::SocketAddr,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::json;
use tower::{Layer, Service};

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last: Instant,
}

#[derive(Debug)]
struct Inner {
    rps: u32,
    burst: u32,
    buckets: Mutex<HashMap<String, Bucket>>,
}

/// Tower layer enforcing a per-key token bucket.
///
/// Keys are client IPs when known, otherwise a single global bucket (which is
/// what the `oneshot` test harness exercises).
#[derive(Debug, Clone)]
pub struct RateLimitLayer {
    inner: Arc<Inner>,
}

impl RateLimitLayer {
    /// Production global governor: sustained `rps` with `burst` headroom.
    pub fn global(rps: u32, burst: u32) -> Self {
        Self::new(rps.max(1), burst.max(1))
    }

    /// Strict set for key management: 1 rps sustained, burst 10.
    pub fn strict() -> Self {
        Self::new(1, 10)
    }

    fn new(rps: u32, burst: u32) -> Self {
        Self {
            inner: Arc::new(Inner {
                rps,
                burst,
                buckets: Mutex::new(HashMap::new()),
            }),
        }
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            limiter: self.inner.clone(),
        }
    }
}

/// Tower service produced by [`RateLimitLayer`].
#[derive(Debug, Clone)]
pub struct RateLimitService<S> {
    inner: S,
    limiter: Arc<Inner>,
}

fn client_key<B>(req: &Request<B>) -> String {
    if let Some(forwarded) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        let first = forwarded.split(',').next().unwrap_or("").trim();
        if !first.is_empty() {
            return format!("ip:{first}");
        }
    }
    if let Some(real) = req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let real = real.trim();
        if !real.is_empty() {
            return format!("ip:{real}");
        }
    }
    if let Some(addr) = req.extensions().get::<axum::extract::ConnectInfo<SocketAddr>>() {
        return format!("ip:{}", addr.0.ip());
    }
    "global".to_string()
}

fn too_many(retry_after_secs: u64) -> Response {
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        axum::Json(json!({"error": "rate limited"})),
    )
        .into_response();
    if let Ok(value) = retry_after_secs.to_string().parse() {
        response.headers_mut().insert("retry-after", value);
    }
    response
}

impl<S> Service<Request<Body>> for RateLimitService<S>
where
    S: Service<Request<Body>, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = futures_util::future::BoxFuture<'static, Result<Response, Infallible>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let limiter = self.limiter.clone();
        let mut inner = self.inner.clone();
        // `poll_ready` was just observed; swap in a ready service without
        // re-entering the borrow checker dance.
        std::mem::swap(&mut self.inner, &mut inner);
        Box::pin(async move {
            let key = client_key(&req);
            let retry_after = {
                let mut buckets = limiter.buckets.lock().unwrap_or_else(|e| {
                    // A poisoned mutex still holds valid buckets; keep serving.
                    e.into_inner()
                });
                let now = Instant::now();
                let bucket = buckets.entry(key).or_insert_with(|| Bucket {
                    tokens: f64::from(limiter.burst),
                    last: now,
                });
                let elapsed = now.duration_since(bucket.last);
                bucket.tokens = (bucket.tokens + elapsed.as_secs_f64() * f64::from(limiter.rps))
                    .min(f64::from(limiter.burst));
                bucket.last = now;
                if bucket.tokens >= 1.0 {
                    bucket.tokens -= 1.0;
                    None
                } else {
                    let needed = 1.0 - bucket.tokens;
                    let secs =
                        (needed / f64::from(limiter.rps)).ceil().max(1.0) as u64;
                    // Clamp so a zero-rate misconfiguration cannot emit absurd headers.
                    Some(secs.min(60))
                }
            };
            if let Some(secs) = retry_after {
                return Ok(too_many(secs));
            }
            // Inner is infallible by construction (all handlers convert errors).
            inner.call(req).await
        })
    }
}

/// Idle buckets accumulate across IPs over a long-lived process. Prune entries
/// that have been full for a while; called opportunistically, never on the hot
/// path in tests.
#[allow(dead_code)]
pub fn prune_idle(layer: &RateLimitLayer, idle: Duration) {
    let mut buckets = layer.inner.buckets.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    buckets.retain(|_, bucket| {
        now.duration_since(bucket.last) < idle || bucket.tokens < f64::from(layer.inner.burst)
    });
}

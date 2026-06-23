//! Server-side ingress rate-limiting Tower layer.
//!
//! Translates the pure [`traffic`] decision into gRPC: it resolves the inbound method's
//! profile from a [`TrafficRegistry`], extracts a key per the profile's [`Scope`], charges
//! one cell, and either forwards to the handler or short-circuits with a
//! `RESOURCE_EXHAUSTED` status — without ever calling the inner service.
//!
//! # Placement
//!
//! Installed on the **server** via [`crate::grpc::server::GrpcServerBuilder`], inside the
//! trace span so throttle decisions are observable.
//!
//! # `per_caller` and identity
//!
//! `per_caller` keys on the authenticated principal from [`auth_context::current_principal`],
//! which an upstream auth layer must have established *in this task*. When no principal is
//! present (e.g. auth not yet wired, or an unauthenticated call), the layer **degrades to
//! method-level keying** rather than collapsing all callers into one bucket — it still
//! limits, just not per-identity. This is logged at debug.

use std::sync::Arc;
use std::task::{Context, Poll};

use auth_context::current_principal;
use futures::future::BoxFuture;
use infra_config::TrafficRegistry;
use tonic::{body::Body, Status};
use tower::{Layer, Service};
use traffic::{Scope, TrafficDecision};

/// Tower [`Layer`] that rate-limits inbound gRPC requests from a [`TrafficRegistry`].
///
/// Holds an `Option`: when `None` (no `[traffic]` section configured) the layer is a
/// transparent pass-through, so the server's type is identical whether or not limiting is
/// enabled.
#[derive(Clone, Default)]
pub struct TrafficLayer {
    registry: Option<Arc<TrafficRegistry>>,
}

impl TrafficLayer {
    /// A pass-through layer (no limiting). Used when no `[traffic]` section is configured.
    pub fn disabled() -> Self {
        Self { registry: None }
    }

    /// A layer that enforces the profiles in `registry`.
    pub fn new(registry: Arc<TrafficRegistry>) -> Self {
        Self { registry: Some(registry) }
    }
}

impl<S> Layer<S> for TrafficLayer {
    type Service = TrafficService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TrafficService { inner, registry: self.registry.clone() }
    }
}

/// The concrete service produced by [`TrafficLayer`].
#[derive(Clone)]
pub struct TrafficService<S> {
    inner: S,
    registry: Option<Arc<TrafficRegistry>>,
}

impl<S> Service<http::Request<Body>> for TrafficService<S>
where
    S: Service<http::Request<Body>, Response = http::Response<Body>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<S::Response, S::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<Body>) -> Self::Future {
        let Some(registry) = self.registry.as_ref() else {
            // Limiting disabled — straight pass-through.
            return Box::pin(self.inner.call(req));
        };

        let method = req.uri().path();
        let profile = registry.profile_for(method);
        let key = extract_key(profile.scope(), method);

        if let TrafficDecision::Throttle { retry_after } = profile.check(&key) {
            let retry_ms = u64::try_from(retry_after.as_millis()).unwrap_or(u64::MAX);
            tracing::debug!(
                rpc.method = %method,
                retry_after_ms = retry_ms,
                "traffic: request throttled"
            );
            let response = throttle_response(retry_ms);
            return Box::pin(async move { Ok(response) });
        }

        Box::pin(self.inner.call(req))
    }
}

/// Builds the rate-limit key for `method` under `scope`.
///
/// `per_caller` degrades to method-level keying when no principal is bound (see module docs).
fn extract_key(scope: Scope, method: &str) -> String {
    match scope {
        Scope::PerMethod => method.to_owned(),
        Scope::PerCaller => match current_principal() {
            Some(principal) => format!("{method}|{}", principal.user_id().as_str()),
            None => {
                tracing::debug!(
                    rpc.method = %method,
                    "traffic: per_caller profile but no principal — keying per-method"
                );
                method.to_owned()
            }
        },
    }
}

/// A trailers-only gRPC `RESOURCE_EXHAUSTED` response carrying a `retry-after-ms` hint.
fn throttle_response(retry_ms: u64) -> http::Response<Body> {
    let mut status = Status::resource_exhausted("rate limit exceeded");
    if let Ok(value) = retry_ms.to_string().parse() {
        status.metadata_mut().insert("retry-after-ms", value);
    }
    status.into_http()
}

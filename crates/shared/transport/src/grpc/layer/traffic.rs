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
//! # Observability
//!
//! Every throttle *decision* (whether enforced or merely shadowed) increments the
//! `infra_traffic_throttled` counter — surfaced by the Prometheus exporter as
//! `infra_traffic_throttled_total` — labelled by `profile`, `route`, and `status`
//! (`enforced` | `shadow`). This is what makes a shadow-mode pilot legible: you watch the
//! `shadow` series to see what *would* be rejected before flipping `enforce`. Route
//! cardinality is bounded — unbound methods collapse to a single `<unbound>` label so a
//! flood of arbitrary paths can't blow up the time-series database.
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
use opentelemetry::{global, metrics::Counter, KeyValue};
use tonic::{body::Body, Status};
use tower::{Layer, Service};
use traffic::{Scope, TrafficDecision};

/// Instrument name. The Prometheus exporter appends `_total` for monotonic sums, so this
/// surfaces as `infra_traffic_throttled_total`; OTLP/collector backends see it as-is.
const THROTTLE_METRIC: &str = "infra_traffic_throttled";

/// Route label for methods with no explicit binding — bounds metric cardinality.
const UNBOUND_ROUTE: &str = "<unbound>";

/// Builds the throttle counter from the global meter. The global provider is installed by
/// `telemetry::init`; before that (or in tests) this binds to a no-op meter, so `add` is a
/// harmless no-op rather than a panic.
fn throttle_counter() -> Counter<u64> {
    global::meter("transport")
        .u64_counter(THROTTLE_METRIC)
        .with_description(
            "Requests that triggered a rate-limit throttle decision, labelled by \
             profile, route, and status (enforced|shadow).",
        )
        .build()
}

/// Tower [`Layer`] that rate-limits inbound gRPC requests from a [`TrafficRegistry`].
///
/// Holds an `Option`: when `None` (no `[traffic]` section configured) the layer is a
/// transparent pass-through, so the server's type is identical whether or not limiting is
/// enabled.
#[derive(Clone)]
pub struct TrafficLayer {
    registry: Option<Arc<TrafficRegistry>>,
    counter: Counter<u64>,
}

impl TrafficLayer {
    /// A pass-through layer (no limiting). Used when no `[traffic]` section is configured.
    pub fn disabled() -> Self {
        Self { registry: None, counter: throttle_counter() }
    }

    /// A layer that enforces the profiles in `registry`.
    pub fn new(registry: Arc<TrafficRegistry>) -> Self {
        Self { registry: Some(registry), counter: throttle_counter() }
    }
}

impl<S> Layer<S> for TrafficLayer {
    type Service = TrafficService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TrafficService { inner, registry: self.registry.clone(), counter: self.counter.clone() }
    }
}

/// The concrete service produced by [`TrafficLayer`].
#[derive(Clone)]
pub struct TrafficService<S> {
    inner: S,
    registry: Option<Arc<TrafficRegistry>>,
    counter: Counter<u64>,
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
        let (profile_name, bound, profile) = registry.resolve(method);
        let key = extract_key(profile.scope(), method);

        if let TrafficDecision::Throttle { retry_after } = profile.check(&key) {
            let retry_ms = u64::try_from(retry_after.as_millis()).unwrap_or(u64::MAX);
            let enforce = profile.enforce();

            // Record the decision before acting on it, so shadow and enforced are both
            // observable. Unbound routes collapse to one label to bound cardinality.
            let route = if bound { method } else { UNBOUND_ROUTE };
            self.counter.add(1, &throttle_attrs(profile_name, route, enforce));

            if enforce {
                tracing::debug!(
                    rpc.method = %method,
                    retry_after_ms = retry_ms,
                    "traffic: request throttled"
                );
                let response = throttle_response(retry_ms);
                return Box::pin(async move { Ok(response) });
            }

            // Shadow mode: the cell was charged (so the metric is real), but we admit the
            // request instead of rejecting it. This is the observe-before-enforce rail.
            tracing::debug!(
                rpc.method = %method,
                retry_after_ms = retry_ms,
                "traffic: would throttle (shadow mode — admitted)"
            );
        }

        Box::pin(self.inner.call(req))
    }
}

/// Attribute set for the throttle counter. `status` distinguishes a real rejection from a
/// shadow-mode observation; `profile`/`route` scope it.
fn throttle_attrs(profile: &str, route: &str, enforce: bool) -> [KeyValue; 3] {
    [
        KeyValue::new("profile", profile.to_string()),
        KeyValue::new("route", route.to_string()),
        KeyValue::new("status", if enforce { "enforced" } else { "shadow" }),
    ]
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

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::Value;

    fn has(attrs: &[KeyValue], key: &str, val: &str) -> bool {
        attrs
            .iter()
            .any(|kv| kv.key.as_str() == key && kv.value == Value::from(val.to_string()))
    }

    #[test]
    fn attrs_carry_profile_route_and_enforced_status() {
        let attrs = throttle_attrs("write-tight", "/post.PostService/CreatePost", true);
        assert!(has(&attrs, "profile", "write-tight"));
        assert!(has(&attrs, "route", "/post.PostService/CreatePost"));
        assert!(has(&attrs, "status", "enforced"));
    }

    #[test]
    fn attrs_distinguish_shadow_and_bounded_route() {
        let attrs = throttle_attrs("standard", UNBOUND_ROUTE, false);
        assert!(has(&attrs, "status", "shadow"));
        assert!(has(&attrs, "route", "<unbound>"));
    }
}

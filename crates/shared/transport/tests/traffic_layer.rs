//! gRPC ingress rate-limit layer: short-circuits with RESOURCE_EXHAUSTED, passes through
//! when disabled. Drives `TrafficService` as a plain tower service (no live server needed).

use std::convert::Infallible;
use std::sync::Arc;

use http::header::HeaderName;
use infra_config::{InfrastructureConfig, TrafficRegistry};
use tonic::body::Body;
use tower::{Layer, ServiceExt};
use transport::grpc::layer::TrafficLayer;

/// Edge-mesh identity header the tests inject (matches the transport default).
const ID_HEADER: &str = "x-edge-user";

/// Builds an enforcing layer with the default identity header.
fn layer(registry: Arc<TrafficRegistry>) -> TrafficLayer {
    TrafficLayer::new(registry, HeaderName::from_static(ID_HEADER))
}

const TOML: &str = r#"
[resilience]
default_profile = "standard"
[resilience.profiles.standard]
timeout = { duration_ms = 10000 }
circuit_breaker = { failure_threshold = 5, success_threshold = 2, open_duration_ms = 30000, half_open_max_calls = 1 }
retry = { max_attempts = 3, backoff = { kind = "exponential", base_ms = 50, max_ms = 10000, jitter = "full" } }

[traffic]
default_profile = "standard"
[traffic.profiles.standard]
rps = 1
burst = 1
scope = "per_method"
"#;

fn registry() -> Arc<TrafficRegistry> {
    let cfg = InfrastructureConfig::from_toml(TOML).unwrap();
    Arc::new(TrafficRegistry::from_section(cfg.traffic.unwrap()).unwrap())
}

/// The inner gRPC service stand-in: always 200, no gRPC error status. Defined as a macro so
/// each call site keeps the concrete `ServiceFn` type (and its `Send + 'static` auto-traits,
/// which `TrafficService`'s `Service` impl requires).
macro_rules! ok_service {
    () => {
        tower::service_fn(|_req: http::Request<Body>| async {
            Ok::<_, Infallible>(http::Response::new(Body::empty()))
        })
    };
}

fn req(path: &str) -> http::Request<Body> {
    http::Request::builder().uri(path).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn allows_then_throttles_with_resource_exhausted() {
    let svc = layer(registry()).layer(ok_service!());

    // 1st: admitted → forwarded to inner, no gRPC error status.
    let resp = svc.clone().oneshot(req("/svc/M")).await.unwrap();
    assert!(resp.headers().get("grpc-status").is_none(), "first call should pass through");

    // 2nd (same method key): burst exhausted → short-circuit, inner never called.
    let resp = svc.clone().oneshot(req("/svc/M")).await.unwrap();
    assert_eq!(
        resp.headers().get("grpc-status").and_then(|v| v.to_str().ok()),
        Some("8"), // RESOURCE_EXHAUSTED
    );
    assert!(resp.headers().get("retry-after-ms").is_some(), "retry hint present");
}

#[tokio::test]
async fn distinct_methods_have_separate_buckets() {
    let svc = layer(registry()).layer(ok_service!());

    let a = svc.clone().oneshot(req("/svc/A")).await.unwrap();
    let b = svc.clone().oneshot(req("/svc/B")).await.unwrap();
    assert!(a.headers().get("grpc-status").is_none());
    assert!(b.headers().get("grpc-status").is_none());
}

#[tokio::test]
async fn shadow_mode_admits_despite_exceeding_quota() {
    // Same tight quota, but enforce = false: would-throttle is observed, not acted on.
    let shadow_toml = TOML.replace(
        "scope = \"per_method\"",
        "scope = \"per_method\"\nenforce = false",
    );
    let cfg = InfrastructureConfig::from_toml(&shadow_toml).unwrap();
    let registry = Arc::new(TrafficRegistry::from_section(cfg.traffic.unwrap()).unwrap());
    let svc = layer(registry).layer(ok_service!());

    // Both calls are admitted even though the second exceeds burst = 1.
    for _ in 0..3 {
        let resp = svc.clone().oneshot(req("/svc/M")).await.unwrap();
        assert!(
            resp.headers().get("grpc-status").is_none(),
            "shadow mode must admit, never short-circuit"
        );
    }
}

#[tokio::test]
async fn disabled_layer_is_passthrough() {
    let svc = TrafficLayer::disabled().layer(ok_service!());
    for _ in 0..5 {
        let resp = svc.clone().oneshot(req("/svc/M")).await.unwrap();
        assert!(resp.headers().get("grpc-status").is_none());
    }
}

// ── per_caller (edge-mesh identity) ───────────────────────────────────────────

const PER_CALLER_TOML: &str = r#"
[resilience]
default_profile = "standard"
[resilience.profiles.standard]
timeout = { duration_ms = 10000 }
circuit_breaker = { failure_threshold = 5, success_threshold = 2, open_duration_ms = 30000, half_open_max_calls = 1 }
retry = { max_attempts = 3, backoff = { kind = "exponential", base_ms = 50, max_ms = 10000, jitter = "full" } }

[traffic]
default_profile = "tight"
[traffic.profiles.tight]
rps = 1
burst = 1
scope = "per_caller"
"#;

fn per_caller_registry() -> Arc<TrafficRegistry> {
    let cfg = InfrastructureConfig::from_toml(PER_CALLER_TOML).unwrap();
    Arc::new(TrafficRegistry::from_section(cfg.traffic.unwrap()).unwrap())
}

fn req_with_identity(path: &str, id: &str) -> http::Request<Body> {
    http::Request::builder().uri(path).header(ID_HEADER, id).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn per_caller_isolates_distinct_identities() {
    let svc = layer(per_caller_registry()).layer(ok_service!());

    // Two different edge identities on the same method each get their own bucket.
    let alice = svc.clone().oneshot(req_with_identity("/svc/M", "alice")).await.unwrap();
    let bob = svc.clone().oneshot(req_with_identity("/svc/M", "bob")).await.unwrap();
    assert!(alice.headers().get("grpc-status").is_none());
    assert!(bob.headers().get("grpc-status").is_none());

    // Alice again → only alice's bucket is exhausted (bob is unaffected above).
    let alice_again = svc.clone().oneshot(req_with_identity("/svc/M", "alice")).await.unwrap();
    assert_eq!(
        alice_again.headers().get("grpc-status").and_then(|v| v.to_str().ok()),
        Some("8"),
    );
}

#[tokio::test]
async fn per_caller_without_identity_falls_back_to_method_bucket() {
    let svc = layer(per_caller_registry()).layer(ok_service!());

    // No edge identity header → unauthenticated traffic shares the method-level bucket
    // (still limited, just not per-identity).
    let first = svc.clone().oneshot(req("/svc/M")).await.unwrap();
    assert!(first.headers().get("grpc-status").is_none());
    let second = svc.clone().oneshot(req("/svc/M")).await.unwrap();
    assert_eq!(
        second.headers().get("grpc-status").and_then(|v| v.to_str().ok()),
        Some("8"),
    );
}

// ── distributed mode (Step 2 backend) ─────────────────────────────────────────

use std::time::Duration;

use async_trait::async_trait;
use traffic::{Quota, QuotaBackend, QuotaError, TrafficDecision};

/// Fake distributed backend: returns a fixed outcome (or simulates being unreachable),
/// standing in for `traffic-redis` so the layer's decision path + fail policy are testable
/// without Redis.
enum Fake {
    Allow,
    Throttle,
    Down,
}

struct FakeBackend(Fake);

#[async_trait]
impl QuotaBackend for FakeBackend {
    async fn check(&self, _key: &str, _quota: Quota) -> Result<TrafficDecision, QuotaError> {
        match self.0 {
            Fake::Allow => Ok(TrafficDecision::Allow),
            Fake::Throttle => Ok(TrafficDecision::Throttle { retry_after: Duration::from_millis(100) }),
            Fake::Down => Err(QuotaError("backend down".into())),
        }
    }
}

/// Distributed `[traffic]` registry with rps=1/burst=1 (so the *local* fallback governor
/// throttles after one request), optionally setting `on_backend_error`.
fn dist_registry(on_backend_error: Option<&str>) -> Arc<TrafficRegistry> {
    let policy = on_backend_error
        .map(|p| format!("\non_backend_error = \"{p}\""))
        .unwrap_or_default();
    let toml = format!(
        r#"
[resilience]
default_profile = "standard"
[resilience.profiles.standard]
timeout = {{ duration_ms = 10000 }}
circuit_breaker = {{ failure_threshold = 5, success_threshold = 2, open_duration_ms = 30000, half_open_max_calls = 1 }}
retry = {{ max_attempts = 3, backoff = {{ kind = "exponential", base_ms = 50, max_ms = 10000, jitter = "full" }} }}

[traffic]
default_profile = "dist"
[traffic.profiles.dist]
rps = 1
burst = 1
scope = "per_method"
mode = "distributed"
lease_ms = 1000{policy}
"#
    );
    let cfg = InfrastructureConfig::from_toml(&toml).unwrap();
    Arc::new(TrafficRegistry::from_section(cfg.traffic.unwrap()).unwrap())
}

fn backend(fake: Fake) -> Arc<dyn QuotaBackend> {
    Arc::new(FakeBackend(fake))
}

#[tokio::test]
async fn distributed_backend_throttle_short_circuits() {
    let svc = layer(dist_registry(None)).with_backend(backend(Fake::Throttle)).layer(ok_service!());
    let resp = svc.clone().oneshot(req("/svc/M")).await.unwrap();
    assert_eq!(resp.headers().get("grpc-status").and_then(|v| v.to_str().ok()), Some("8"));
}

#[tokio::test]
async fn distributed_backend_allow_passes_through() {
    let svc = layer(dist_registry(None)).with_backend(backend(Fake::Allow)).layer(ok_service!());
    // The local governor would throttle after 1; the global backend authorises every request.
    for _ in 0..5 {
        let resp = svc.clone().oneshot(req("/svc/M")).await.unwrap();
        assert!(resp.headers().get("grpc-status").is_none());
    }
}

#[tokio::test]
async fn distributed_fail_closed_rejects_when_backend_down() {
    let svc = layer(dist_registry(Some("fail_closed")))
        .with_backend(backend(Fake::Down))
        .layer(ok_service!());
    let resp = svc.clone().oneshot(req("/svc/M")).await.unwrap();
    assert_eq!(resp.headers().get("grpc-status").and_then(|v| v.to_str().ok()), Some("8"));
}

#[tokio::test]
async fn distributed_fail_open_degrades_to_local_governor() {
    // fail_open + backend down → fall back to the local governor (rps=1, burst=1).
    let svc = layer(dist_registry(Some("fail_open")))
        .with_backend(backend(Fake::Down))
        .layer(ok_service!());
    let first = svc.clone().oneshot(req("/svc/M")).await.unwrap();
    assert!(first.headers().get("grpc-status").is_none(), "local governor admits the first");
    let second = svc.clone().oneshot(req("/svc/M")).await.unwrap();
    assert_eq!(
        second.headers().get("grpc-status").and_then(|v| v.to_str().ok()),
        Some("8"),
        "local governor throttles the second",
    );
}

#[tokio::test]
async fn distributed_without_backend_degrades_to_local() {
    // distributed profile but no backend wired → local governor is the (degraded) limiter.
    let svc = layer(dist_registry(None)).layer(ok_service!());
    let first = svc.clone().oneshot(req("/svc/M")).await.unwrap();
    assert!(first.headers().get("grpc-status").is_none());
    let second = svc.clone().oneshot(req("/svc/M")).await.unwrap();
    assert_eq!(second.headers().get("grpc-status").and_then(|v| v.to_str().ok()), Some("8"));
}

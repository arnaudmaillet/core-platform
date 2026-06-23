//! gRPC ingress rate-limit layer: short-circuits with RESOURCE_EXHAUSTED, passes through
//! when disabled. Drives `TrafficService` as a plain tower service (no live server needed).

use std::convert::Infallible;
use std::sync::Arc;

use infra_config::{InfrastructureConfig, TrafficRegistry};
use tonic::body::Body;
use tower::{Layer, ServiceExt};
use transport::grpc::layer::TrafficLayer;

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
    let svc = TrafficLayer::new(registry()).layer(ok_service!());

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
    let svc = TrafficLayer::new(registry()).layer(ok_service!());

    let a = svc.clone().oneshot(req("/svc/A")).await.unwrap();
    let b = svc.clone().oneshot(req("/svc/B")).await.unwrap();
    assert!(a.headers().get("grpc-status").is_none());
    assert!(b.headers().get("grpc-status").is_none());
}

#[tokio::test]
async fn disabled_layer_is_passthrough() {
    let svc = TrafficLayer::disabled().layer(ok_service!());
    for _ in 0..5 {
        let resp = svc.clone().oneshot(req("/svc/M")).await.unwrap();
        assert!(resp.headers().get("grpc-status").is_none());
    }
}

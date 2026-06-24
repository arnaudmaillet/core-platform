use opentelemetry::propagation::{Extractor, Injector};

/// Mutable carrier that injects W3C TraceContext headers into an outgoing HTTP request's
/// [`http::HeaderMap`].
///
/// Used by [`crate::grpc::layer::outbound::OutboundTraceService`] before forwarding
/// every client-side gRPC call.
pub struct GrpcHeaderInjector<'a>(pub &'a mut http::HeaderMap);

impl Injector for GrpcHeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        match (
            http::header::HeaderName::from_bytes(key.as_bytes()),
            http::header::HeaderValue::from_str(&value),
        ) {
            (Ok(name), Ok(val)) => {
                self.0.insert(name, val);
            }
            _ => {
                tracing::warn!(
                    key,
                    value,
                    "skipping non-ASCII trace header during gRPC injection"
                );
            }
        }
    }
}

/// Read-only carrier that extracts W3C TraceContext headers from an incoming HTTP request's
/// [`http::HeaderMap`].
///
/// Used by [`crate::grpc::layer::inbound::InboundTraceService`] on every server-side
/// gRPC call.
pub struct GrpcHeaderExtractor<'a>(pub &'a http::HeaderMap);

impl Extractor for GrpcHeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(http::header::HeaderName::as_str).collect()
    }
}

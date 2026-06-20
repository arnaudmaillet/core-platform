use std::task::{Context, Poll};

use tower::{Layer, Service};

use crate::propagation::{carrier::inject_context, grpc::GrpcHeaderInjector};

/// Tower [`Layer`] that injects the active OpenTelemetry trace context into every
/// outgoing gRPC request's HTTP headers before forwarding to the inner service.
///
/// # Placement in the stack
///
/// Apply this layer directly around the raw [`tonic::transport::Channel`] so that
/// trace headers are injected on every attempt, including retries:
///
/// ```text
/// TimeoutLayer
///   └─ CircuitBreakerLayer
///       └─ OutboundTraceLayer   ← here
///           └─ tonic::transport::Channel
/// ```
///
/// # Context source
///
/// The injected context is read from the **current `tracing` span** at call time via
/// `tracing_opentelemetry::OpenTelemetrySpanExt::context`. The active span must exist
/// (i.e., `telemetry::init()` has been called and an instrumented task is running);
/// if no span is active the propagator will inject an empty context.
#[derive(Debug, Clone, Default)]
pub struct OutboundTraceLayer;

impl<S> Layer<S> for OutboundTraceLayer {
    type Service = OutboundTraceService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        OutboundTraceService { inner }
    }
}

/// The concrete service produced by [`OutboundTraceLayer`].
#[derive(Debug, Clone)]
pub struct OutboundTraceService<S> {
    inner: S,
}

impl<S, B> Service<http::Request<B>> for OutboundTraceService<S>
where
    S: Service<http::Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<B>) -> Self::Future {
        // Inject the current span's trace context into the request headers.
        // All work is synchronous so the future type is unchanged — no boxing required.
        inject_context(&mut GrpcHeaderInjector(req.headers_mut()));
        self.inner.call(req)
    }
}

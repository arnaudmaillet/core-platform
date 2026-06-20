use std::task::{Context, Poll};

use futures::future::BoxFuture;
use tower::{Layer, Service};
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::propagation::{carrier::extract_context, grpc::GrpcHeaderExtractor};

/// Tower [`Layer`] that extracts the remote OpenTelemetry trace context from every
/// incoming gRPC request and wires it as the parent span of the current `tracing` span.
///
/// # Placement in the stack
///
/// Apply this layer on the **server side** via [`tonic::transport::Server::layer`]:
///
/// ```text
/// tonic::Server
///   └─ InboundTraceLayer   ← wraps every handler
///       └─ generated gRPC service impl
/// ```
///
/// # Span attributes
///
/// The injected span carries:
/// - `rpc.system = "grpc"`
/// - `rpc.method` = the full URI path (e.g. `/social.PostService/CreatePost`)
///
/// These align with the [OpenTelemetry RPC semantic conventions](https://opentelemetry.io/docs/specs/semconv/rpc/).
#[derive(Debug, Clone, Default)]
pub struct InboundTraceLayer;

impl<S> Layer<S> for InboundTraceLayer {
    type Service = InboundTraceService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        InboundTraceService { inner }
    }
}

/// The concrete service produced by [`InboundTraceLayer`].
pub struct InboundTraceService<S> {
    inner: S,
}

impl<S, B> Service<http::Request<B>> for InboundTraceService<S>
where
    S: Service<http::Request<B>> + Send + 'static,
    S::Future: Send + 'static,
    S::Response: Send + 'static,
    S::Error: Send + 'static,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    // BoxFuture because we instrument the inner future with a new span, changing its type.
    type Future = BoxFuture<'static, Result<S::Response, S::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        // 1. Extract the remote trace context from the request headers.
        let parent_cx = extract_context(&GrpcHeaderExtractor(req.headers()));

        // 2. Build a server-side span with the remote context as parent.
        //    Using `info_span!` so that sampling decisions from the upstream propagate.
        let method = req.uri().path().to_owned();
        let span = tracing::info_span!(
            "grpc.server",
            rpc.system = "grpc",
            rpc.method = %method,
        );
        span.set_parent(parent_cx);

        // 3. Instrument the inner future so the span covers the full handler execution.
        //    `S::Future: Send + 'static` ensures the resulting BoxFuture is moveable
        //    across task boundaries without holding a reference into `self`.
        Box::pin(self.inner.call(req).instrument(span))
    }
}

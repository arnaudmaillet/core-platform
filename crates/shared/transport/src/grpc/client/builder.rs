use tower::Layer;

use crate::{
    error::TransportError,
    grpc::{
        client::config::GrpcClientConfig,
        layer::outbound::{OutboundTraceLayer, OutboundTraceService},
    },
};

/// Builds a Tonic gRPC client channel, with optional TLS and outbound trace injection.
///
/// # Two entry points
///
/// | Method | Returns | Cloneable | Middleware |
/// |--------|---------|-----------|-----------|
/// | `connect()` | raw `Channel` | ✅ | none |
/// | `build_traced()` | `OutboundTraceService<Channel>` | ✅ | trace injection only |
///
/// # Stacking resilience layers
///
/// HTTP/2 request bodies are streams — they cannot be replayed without buffering, so
/// `RetryLayer` is intentionally absent at the transport level. Apply it at the RPC
/// application layer instead.  To add `CircuitBreakerLayer` + `TimeoutLayer` on top
/// of a traced channel:
///
/// ```rust,ignore
/// use tower::ServiceBuilder;
/// use resilience::{circuit_breaker::layer::CircuitBreakerLayer, timeout::layer::TimeoutLayer};
///
/// let channel = GrpcClientBuilder::new(cfg).build_traced().await?;
///
/// let svc = ServiceBuilder::new()
///     .map_err(TransportError::from_resilience)
///     .layer(TimeoutLayer::new(resilience_cfg.timeout))
///     .map_err(TransportError::from_resilience_connect)
///     .layer(CircuitBreakerLayer::new(resilience_cfg.circuit_breaker))
///     .service(channel);
///
/// let client = PostServiceClient::new(svc);
/// ```
pub struct GrpcClientBuilder {
    config: GrpcClientConfig,
}

impl GrpcClientBuilder {
    pub fn new(config: GrpcClientConfig) -> Self {
        Self { config }
    }

    /// Connects to the remote endpoint and returns a raw, cloneable
    /// [`tonic::transport::Channel`].
    ///
    /// No middleware is applied. Prefer this when you need to share the channel across
    /// multiple tonic client types, or when you want full control over the Tower stack.
    pub async fn connect(self) -> Result<tonic::transport::Channel, TransportError> {
        build_channel(&self.config).await
    }

    /// Connects and wraps the channel in [`OutboundTraceLayer`], which automatically
    /// injects W3C `traceparent` / `tracestate` headers on every outgoing gRPC call.
    ///
    /// The returned service is [`Clone`] (both `Channel` and `OutboundTraceService` are
    /// cheaply cloneable) and can be used directly with generated tonic clients:
    ///
    /// ```rust,ignore
    /// let svc = GrpcClientBuilder::new(config).build_traced().await?;
    /// let client = PostServiceClient::new(svc);
    /// ```
    pub async fn build_traced(
        self,
    ) -> Result<OutboundTraceService<tonic::transport::Channel>, TransportError> {
        let channel = build_channel(&self.config).await?;
        Ok(OutboundTraceLayer.layer(channel))
    }
}

async fn build_channel(cfg: &GrpcClientConfig) -> Result<tonic::transport::Channel, TransportError> {
    use tonic::transport::{Certificate, ClientTlsConfig, Endpoint, Identity};

    let mut endpoint = Endpoint::new(cfg.endpoint.clone())
        .map_err(|e| TransportError::Grpc(crate::grpc::error::GrpcTransportError::Connect(e)))?
        .connect_timeout(cfg.connect_timeout);

    if let Some(tls) = &cfg.tls {
        let mut tls_config = ClientTlsConfig::new();

        if let Some(ca_pem) = &tls.ca_pem {
            tls_config = tls_config.ca_certificate(Certificate::from_pem(ca_pem));
        }

        if let Some(id) = &tls.identity {
            tls_config =
                tls_config.identity(Identity::from_pem(&id.cert_pem, &id.key_pem));
        }

        endpoint = endpoint.tls_config(tls_config).map_err(|e| {
            TransportError::Grpc(crate::grpc::error::GrpcTransportError::Tls(e.to_string()))
        })?;
    }

    endpoint.connect().await.map_err(TransportError::from)
}

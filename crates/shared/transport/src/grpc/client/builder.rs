use resilience::{error::ResilienceError, ResilienceProfile};
use resilience_config::ResilienceRegistry;
use tonic::transport::Channel;
use tower::{util::BoxCloneService, Layer, ServiceBuilder};

use crate::{
    error::TransportError,
    grpc::{
        client::config::GrpcClientConfig,
        layer::outbound::{OutboundTraceLayer, OutboundTraceService},
    },
};

/// A fully-composed, cloneable gRPC client stack — trace injection + circuit breaker +
/// timeout — type-erased and flattened to a single [`TransportError`].
///
/// Plugs straight into a generated tonic client: `PostServiceClient::new(channel)`.
/// Because the circuit-breaker and timeout layers read their config from the originating
/// [`ResilienceProfile`]'s shared handles, a control-plane hot-swap reconfigures this live
/// stack with no rebuild.
pub type ResilientChannel = BoxCloneService<
    http::Request<tonic::body::Body>,
    http::Response<tonic::body::Body>,
    TransportError,
>;

/// Composes the resilience stack over a connected channel.
///
/// Layer order (outermost → innermost) matches [`OutboundTraceLayer`]'s documented placement:
/// `Timeout → CircuitBreaker → OutboundTrace → Channel`. The interleaved `map_err`s flatten
/// each layer's `ResilienceError<_>` back into `TransportError` so the erased service exposes
/// one error type. Function-pointer mappers keep the whole stack `Clone`.
fn compose_resilient(channel: Channel, profile: &ResilienceProfile) -> ResilientChannel {
    let traced = OutboundTraceLayer.layer(channel);

    let svc = ServiceBuilder::new()
        .map_err(
            TransportError::from_resilience
                as fn(ResilienceError<TransportError>) -> TransportError,
        )
        .layer(profile.timeout_layer())
        .map_err(
            TransportError::from_resilience_connect
                as fn(ResilienceError<tonic::transport::Error>) -> TransportError,
        )
        .layer(profile.circuit_breaker_layer())
        .service(traced);

    BoxCloneService::new(svc)
}

/// Builds a Tonic gRPC client channel, with optional TLS and outbound trace injection.
///
/// # Two entry points
///
/// | Method | Returns | Cloneable | Middleware |
/// |--------|---------|-----------|-----------|
/// | `connect()` | raw `Channel` | ✅ | none |
/// | `build_traced()` | `OutboundTraceService<Channel>` | ✅ | trace injection only |
/// | `build_resilient(&profile)` | [`ResilientChannel`] | ✅ | trace + circuit breaker + timeout |
/// | `build_from_registry(&registry)` | [`ResilientChannel`] | ✅ | resolves the profile from bindings, then as above |
///
/// # Resilience
///
/// `build_resilient` / `build_from_registry` compose the full stack for you and erase it to
/// a single [`ResilientChannel`] that drops straight into a generated tonic client. The
/// circuit-breaker and timeout configs come from a [`ResilienceProfile`] (resolved from
/// `infrastructure.toml` bindings via the registry), so a control-plane hot-swap reconfigures
/// the live channel without a rebuild.
///
/// `RetryLayer` is intentionally absent at the transport level: HTTP/2 request bodies are
/// streams that can't be replayed without buffering. Apply retry at the RPC application layer.
///
/// ```rust,ignore
/// let registry = std::sync::Arc::new(ResilienceRegistry::from_config(infra_cfg)?);
/// let _watcher = resilience_config::spawn_watcher(path, std::sync::Arc::clone(&registry))?;
///
/// let cfg = GrpcClientConfig::new("https://post:50051").with_dependency("post-command");
/// let channel = GrpcClientBuilder::new(cfg).build_from_registry(&registry).await?;
/// let client = PostServiceClient::new(channel);
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

    /// Connects and wraps the channel in the full resilience stack driven by `profile`
    /// (trace injection + circuit breaker + timeout). The returned [`ResilientChannel`] is
    /// cloneable, hot-reloadable, and ready to hand to a generated tonic client.
    ///
    /// ```rust,ignore
    /// let profile = registry.profile_for("post-command");
    /// let channel = GrpcClientBuilder::new(cfg).build_resilient(&profile).await?;
    /// let client = PostServiceClient::new(channel);
    /// ```
    pub async fn build_resilient(
        self,
        profile: &ResilienceProfile,
    ) -> Result<ResilientChannel, TransportError> {
        let channel = build_channel(&self.config).await?;
        Ok(compose_resilient(channel, profile))
    }

    /// Resolves this client's resilience profile from the registry — keyed by
    /// [`GrpcClientConfig::dependency`] (its `[resilience.bindings]` entry, falling back to
    /// the default profile) — and builds the resilient channel from it.
    ///
    /// This is the registry-driven entry point: bindings and profiles come from
    /// `infrastructure.toml`, and the resulting stack hot-reloads with it.
    ///
    /// ```rust,ignore
    /// let registry = Arc::new(ResilienceRegistry::from_config(cfg)?);
    /// let _watcher = resilience_config::spawn_watcher(path, Arc::clone(&registry))?;
    ///
    /// let channel = GrpcClientBuilder::new(GrpcClientConfig::new(uri).with_dependency("post-command"))
    ///     .build_from_registry(&registry)
    ///     .await?;
    /// ```
    pub async fn build_from_registry(
        self,
        registry: &ResilienceRegistry,
    ) -> Result<ResilientChannel, TransportError> {
        let profile = registry.profile_for(&self.config.dependency);
        self.build_resilient(&profile).await
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

#[cfg(test)]
mod tests {
    use resilience_config::{InfrastructureConfig, ResilienceRegistry};

    use super::*;

    const TOML: &str = r#"
[resilience]
default_profile = "standard"
[resilience.profiles.standard]
timeout = { duration_ms = 10000 }
circuit_breaker = { failure_threshold = 5, success_threshold = 2, open_duration_ms = 30000, half_open_max_calls = 1 }
retry = { max_attempts = 3, backoff = { kind = "exponential", base_ms = 50, max_ms = 10000, jitter = "full" } }
[resilience.profiles.critical]
timeout = { duration_ms = 2000 }
circuit_breaker = { failure_threshold = 5, success_threshold = 2, open_duration_ms = 30000, half_open_max_calls = 1 }
retry = { max_attempts = 1, backoff = { kind = "exponential", base_ms = 20, max_ms = 500, jitter = "full" } }
[resilience.bindings]
"post-command" = "critical"
"#;

    #[tokio::test]
    async fn composes_resilient_channel_bound_to_registry_profile() {
        let registry =
            ResilienceRegistry::from_config(InfrastructureConfig::from_toml(TOML).unwrap()).unwrap();

        // "post-command" is bound to the "critical" profile (2s timeout).
        let profile = registry.profile_for("post-command");
        assert_eq!(profile.timeout.load().duration.as_millis(), 2000);

        // Lazy channel — composes the full stack without needing a live server.
        let channel = Channel::from_static("http://127.0.0.1:50051").connect_lazy();
        let svc: ResilientChannel = compose_resilient(channel, &profile);

        // Must be cloneable: tonic clones the service per RPC.
        let _clone = svc.clone();

        // The live stack tracks the registry: an incident-time hot-swap on the bound
        // profile is observed through the handle the composed layers hold.
        let tightened = TOML.replace("duration_ms = 2000", "duration_ms = 250");
        registry
            .apply(InfrastructureConfig::from_toml(&tightened).unwrap())
            .unwrap();
        assert_eq!(profile.timeout.load().duration.as_millis(), 250);
    }
}

use resilience::{error::ResilienceError, ResilienceProfile};
use infra_config::ResilienceRegistry;
use tonic::transport::Channel;
use tower::{Layer, ServiceBuilder};

use crate::{
    error::TransportError,
    grpc::{
        client::{
            config::GrpcClientConfig,
            sync_box::BoxCloneSyncService,
        },
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
///
/// The erasure is [`Send`] **and** [`Sync`] (via [`BoxCloneSyncService`]): a client adapter
/// typically stores it behind an `Arc` and shares it across worker tasks, and `Arc<T>: Sync`
/// requires `T: Sync`.
pub type ResilientChannel = BoxCloneSyncService<
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

    BoxCloneSyncService::new(svc)
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
/// `build_resilient` / `build_from_registry` connect eagerly; their `*_lazy` counterparts
/// compose the identical stack over a lazily-connected channel for callers that must not
/// block boot on the dependency.
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
/// let _watcher = infra_config::spawn_watcher(path, std::sync::Arc::clone(&registry))?;
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

    /// Lazy counterpart to [`build_resilient`](Self::build_resilient): composes the same
    /// stack over a [`connect_lazy`](tonic::transport::Endpoint::connect_lazy) channel, so
    /// the connection is established on the first RPC instead of up front.
    ///
    /// Prefer this when the caller must boot even if the dependency is unreachable — the
    /// channel is built without a network round-trip, and connect failures surface on the
    /// first call (where the circuit breaker and timeout already wrap them) rather than at
    /// construction. It is therefore synchronous: only endpoint/TLS parsing can fail here.
    pub fn build_resilient_lazy(
        self,
        profile: &ResilienceProfile,
    ) -> Result<ResilientChannel, TransportError> {
        let channel = build_channel_lazy(&self.config)?;
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
    /// let _watcher = infra_config::spawn_watcher(path, Arc::clone(&registry))?;
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

    /// Lazy counterpart to [`build_from_registry`](Self::build_from_registry): resolves the
    /// profile binding exactly the same way, but composes the stack over a lazily-connected
    /// channel (see [`build_resilient_lazy`](Self::build_resilient_lazy)).
    ///
    /// This is the entry point for a caller that must not block boot on the dependency —
    /// e.g. a read-path service whose downstream is only needed on cold-rebuild.
    ///
    /// ```rust,ignore
    /// let channel = GrpcClientBuilder::new(GrpcClientConfig::new(uri).with_dependency("social-graph"))
    ///     .build_from_registry_lazy(&infra.resilience())?;
    /// let client = SocialGraphServiceClient::new(channel);
    /// ```
    pub fn build_from_registry_lazy(
        self,
        registry: &ResilienceRegistry,
    ) -> Result<ResilientChannel, TransportError> {
        let profile = registry.profile_for(&self.config.dependency);
        self.build_resilient_lazy(&profile)
    }
}

/// Builds a configured [`Endpoint`] (URI, connect timeout, optional TLS) without connecting.
fn build_endpoint(cfg: &GrpcClientConfig) -> Result<tonic::transport::Endpoint, TransportError> {
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

    Ok(endpoint)
}

/// Connects eagerly: the returned channel is backed by an established connection.
async fn build_channel(cfg: &GrpcClientConfig) -> Result<tonic::transport::Channel, TransportError> {
    build_endpoint(cfg)?.connect().await.map_err(TransportError::from)
}

/// Builds a lazily-connected channel: no network round-trip here; the connection is opened
/// on first use. Only endpoint/TLS parsing can fail.
fn build_channel_lazy(cfg: &GrpcClientConfig) -> Result<tonic::transport::Channel, TransportError> {
    Ok(build_endpoint(cfg)?.connect_lazy())
}

#[cfg(test)]
mod tests {
    use infra_config::{InfrastructureConfig, ResilienceRegistry};

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

    // A `ResilientChannel` is stored behind an `Arc` in client adapters that are shared
    // across worker tasks, so it must be `Send + Sync` (not just `Send`). This locks in the
    // `BoxCloneSyncService` erasure — a plain `BoxCloneService` would fail to compile here.
    #[test]
    fn resilient_channel_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ResilientChannel>();
    }

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

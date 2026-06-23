use std::sync::Arc;

use tower::layer::util::{Identity, Stack};
use tonic::transport::{Identity as TlsIdentity, Server, ServerTlsConfig};

use infra_config::TrafficRegistry;
use traffic::QuotaBackend;

use crate::{
    error::TransportError,
    grpc::{
        layer::{inbound::InboundTraceLayer, traffic::TrafficLayer},
        server::config::GrpcServerConfig,
    },
};

/// Concrete server type produced by [`GrpcServerBuilder::build`].
///
/// The tonic type after applying [`InboundTraceLayer`] then [`TrafficLayer`]: trace is the
/// outer layer (so throttled requests are still traced), rate-limiting the inner. The
/// [`TrafficLayer`] is always present in the type — it's a transparent pass-through unless a
/// registry was supplied via [`GrpcServerBuilder::with_traffic`], keeping the return type
/// stable regardless of whether limiting is enabled.
pub type TracedGrpcServer = Server<Stack<TrafficLayer, Stack<InboundTraceLayer, Identity>>>;

/// Builds a Tonic gRPC server with [`InboundTraceLayer`] and [`TrafficLayer`] pre-installed.
///
/// Every request has its W3C TraceContext extracted and linked as the parent span; if a
/// traffic registry was supplied, it is also rate-limited per the bound `[traffic]` profile.
///
/// # Example
///
/// ```rust,ignore
/// let server = GrpcServerBuilder::new(GrpcServerConfig::default())
///     .with_traffic(infra.traffic().expect("[traffic] configured"))
///     .build()?
///     .add_service(PostServiceServer::new(my_handler))
///     .serve(config.addr)
///     .await?;
/// ```
pub struct GrpcServerBuilder {
    config: GrpcServerConfig,
    traffic: Option<Arc<TrafficRegistry>>,
    traffic_backend: Option<Arc<dyn QuotaBackend>>,
}

impl GrpcServerBuilder {
    pub fn new(config: GrpcServerConfig) -> Self {
        Self { config, traffic: None, traffic_backend: None }
    }

    /// Enables ingress rate limiting from the given registry. Without this call the server
    /// installs a transparent (no-op) traffic layer.
    pub fn with_traffic(mut self, registry: Arc<TrafficRegistry>) -> Self {
        self.traffic = Some(registry);
        self
    }

    /// Attaches the distributed-mode coordination backend (e.g. `traffic-redis`). Only
    /// `distributed` profiles use it; without it they degrade to local per-replica limiting.
    /// No effect unless [`with_traffic`](Self::with_traffic) is also set.
    pub fn with_traffic_backend(mut self, backend: Arc<dyn QuotaBackend>) -> Self {
        self.traffic_backend = Some(backend);
        self
    }

    /// Returns a [`TracedGrpcServer`] with the trace and traffic layers applied.
    ///
    /// Call `.add_service(...)` and `.serve(addr)` on the returned server to start
    /// accepting connections.
    pub fn build(self) -> Result<TracedGrpcServer, TransportError> {
        let traffic_layer = match self.traffic {
            Some(registry) => {
                let layer = TrafficLayer::new(registry, self.config.identity_header.clone());
                match self.traffic_backend {
                    Some(backend) => layer.with_backend(backend),
                    None => layer,
                }
            }
            None => TrafficLayer::disabled(),
        };
        // `.layer(InboundTraceLayer)` first makes trace the outer layer; `.layer(traffic)`
        // nests rate-limiting inside the span.
        let mut server = Server::builder().layer(InboundTraceLayer).layer(traffic_layer);

        if let Some(tls) = self.config.tls {
            let identity = TlsIdentity::from_pem(&tls.cert_pem, &tls.key_pem);
            let mut tls_config = ServerTlsConfig::new().identity(identity);

            if let Some(ca) = tls.client_ca_pem {
                tls_config = tls_config
                    .client_ca_root(tonic::transport::Certificate::from_pem(ca));
            }

            server = server.tls_config(tls_config).map_err(|e| {
                TransportError::Grpc(crate::grpc::error::GrpcTransportError::Tls(e.to_string()))
            })?;
        }

        Ok(server)
    }
}

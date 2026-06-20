use tower::layer::util::{Identity, Stack};
use tonic::transport::{Identity as TlsIdentity, Server, ServerTlsConfig};

use crate::{
    error::TransportError,
    grpc::{layer::inbound::InboundTraceLayer, server::config::GrpcServerConfig},
};

/// Concrete server type produced by [`GrpcServerBuilder::build`].
///
/// `Server<Stack<InboundTraceLayer, Identity>>` is the tonic type after calling
/// `Server::builder().layer(InboundTraceLayer)`. Exposed as a public alias so callers
/// can name the type in struct fields or `Arc<>` wrappers.
pub type TracedGrpcServer = Server<Stack<InboundTraceLayer, Identity>>;

/// Builds a Tonic gRPC server with [`InboundTraceLayer`] pre-installed.
///
/// Every request that hits any service registered on this server will have its
/// W3C TraceContext headers extracted and linked as the parent span automatically.
///
/// # Example
///
/// ```rust,ignore
/// let server = GrpcServerBuilder::new(GrpcServerConfig::default())
///     .build()?
///     .add_service(PostServiceServer::new(my_handler))
///     .serve(config.addr)
///     .await?;
/// ```
pub struct GrpcServerBuilder {
    config: GrpcServerConfig,
}

impl GrpcServerBuilder {
    pub fn new(config: GrpcServerConfig) -> Self {
        Self { config }
    }

    /// Returns a [`TracedGrpcServer`] with [`InboundTraceLayer`] applied.
    ///
    /// Call `.add_service(...)` and `.serve(addr)` on the returned server to start
    /// accepting connections.
    pub fn build(self) -> Result<TracedGrpcServer, TransportError> {
        let mut server = Server::builder().layer(InboundTraceLayer);

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

use std::net::SocketAddr;

use http::header::HeaderName;

/// Default inbound header carrying the caller identity, injected by the edge/service mesh.
///
/// **Trust contract:** this header is only safe to key on if the mesh *sets or overwrites*
/// it on every request and *strips* any client-supplied value at the trust boundary. Inside
/// the cluster it is authoritative; never accept it from untrusted ingress.
pub const DEFAULT_IDENTITY_HEADER: &str = "x-edge-user";

/// Configuration for a Tonic gRPC server.
#[derive(Debug, Clone)]
pub struct GrpcServerConfig {
    /// Address to bind. Defaults to `0.0.0.0:50051`.
    pub addr: SocketAddr,

    /// Optional TLS settings. When `None` the server accepts plaintext connections.
    pub tls: Option<GrpcServerTlsConfig>,

    /// Enables gRPC server reflection (useful for `grpcurl` and Postman during development).
    pub enable_reflection: bool,

    /// Inbound header the edge mesh injects with the caller identity. Read by the traffic
    /// layer for `per_caller` rate-limit keying. Defaults to [`DEFAULT_IDENTITY_HEADER`].
    pub identity_header: HeaderName,
}

impl Default for GrpcServerConfig {
    fn default() -> Self {
        Self {
            addr: "0.0.0.0:50051".parse().expect("static address is valid"),
            tls: None,
            enable_reflection: false,
            identity_header: HeaderName::from_static(DEFAULT_IDENTITY_HEADER),
        }
    }
}

impl GrpcServerConfig {
    pub fn with_addr(mut self, addr: SocketAddr) -> Self {
        self.addr = addr;
        self
    }

    pub fn with_tls(mut self, tls: GrpcServerTlsConfig) -> Self {
        self.tls = Some(tls);
        self
    }

    pub fn with_reflection(mut self) -> Self {
        self.enable_reflection = true;
        self
    }

    /// Overrides the edge-mesh identity header used for `per_caller` keying.
    pub fn with_identity_header(mut self, header: HeaderName) -> Self {
        self.identity_header = header;
        self
    }
}

/// TLS settings for a gRPC server.
#[derive(Debug, Clone)]
pub struct GrpcServerTlsConfig {
    /// PEM-encoded server certificate.
    pub cert_pem: Vec<u8>,
    /// PEM-encoded server private key.
    pub key_pem: Vec<u8>,
    /// Optional PEM-encoded CA for client certificate verification (mTLS).
    pub client_ca_pem: Option<Vec<u8>>,
}

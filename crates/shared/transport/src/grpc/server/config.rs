use std::net::SocketAddr;

/// Configuration for a Tonic gRPC server.
#[derive(Debug, Clone)]
pub struct GrpcServerConfig {
    /// Address to bind. Defaults to `0.0.0.0:50051`.
    pub addr: SocketAddr,

    /// Optional TLS settings. When `None` the server accepts plaintext connections.
    pub tls: Option<GrpcServerTlsConfig>,

    /// Enables gRPC server reflection (useful for `grpcurl` and Postman during development).
    pub enable_reflection: bool,
}

impl Default for GrpcServerConfig {
    fn default() -> Self {
        Self {
            addr: "0.0.0.0:50051".parse().expect("static address is valid"),
            tls: None,
            enable_reflection: false,
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

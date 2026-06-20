use std::time::Duration;

use resilience::{
    circuit_breaker::config::CircuitBreakerConfig,
    timeout::config::TimeoutConfig,
};

/// Top-level configuration for a gRPC client channel.
#[derive(Debug, Clone)]
pub struct GrpcClientConfig {
    /// Remote endpoint URI, e.g. `https://post-command-server:50051`.
    pub endpoint: String,

    /// Optional TLS settings. When `None` the connection is plaintext (suitable for
    /// in-cluster service-mesh scenarios where mTLS is handled at the sidecar level).
    pub tls: Option<GrpcTlsConfig>,

    /// How long to wait for the initial TCP + TLS handshake before failing.
    pub connect_timeout: Duration,

    /// Resilience knobs applied at the transport layer. `None` disables all middleware.
    pub resilience: Option<GrpcResilienceConfig>,
}

impl GrpcClientConfig {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            tls: None,
            connect_timeout: Duration::from_secs(5),
            resilience: Some(GrpcResilienceConfig::default()),
        }
    }

    pub fn with_tls(mut self, tls: GrpcTlsConfig) -> Self {
        self.tls = Some(tls);
        self
    }

    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn without_resilience(mut self) -> Self {
        self.resilience = None;
        self
    }
}

/// TLS settings for a gRPC client channel.
#[derive(Debug, Clone)]
pub struct GrpcTlsConfig {
    /// PEM-encoded CA certificate bundle. When `None` the system root store is used.
    pub ca_pem: Option<Vec<u8>>,

    /// Optional client certificate and private key for mutual TLS (mTLS).
    pub identity: Option<GrpcTlsIdentity>,
}

/// Client certificate + key pair for mutual TLS.
#[derive(Debug, Clone)]
pub struct GrpcTlsIdentity {
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
}

/// Resilience middleware configuration for gRPC client channels.
///
/// # Why no `RetryConfig`?
///
/// HTTP/2 request bodies are streams — once consumed they cannot be replayed without
/// buffering the entire body. Retry at the raw transport level would require copying
/// every request body upfront, which is prohibitively expensive for large payloads.
///
/// **Recommended pattern:** implement retry at the application layer (e.g. retry the
/// tonic client call) using the `resilience` crate's `RetryLayer` around the generated
/// gRPC client method, not the underlying channel.
#[derive(Debug, Clone)]
pub struct GrpcResilienceConfig {
    pub circuit_breaker: CircuitBreakerConfig,
    pub timeout: TimeoutConfig,
}

impl Default for GrpcResilienceConfig {
    fn default() -> Self {
        Self {
            circuit_breaker: CircuitBreakerConfig::default(),
            timeout: TimeoutConfig::from_secs(10),
        }
    }
}

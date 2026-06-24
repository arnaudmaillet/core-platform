use thiserror::Error;

#[derive(Debug, Error)]
pub enum GrpcTransportError {
    /// Low-level TCP/TLS connection failure reported by the Tonic transport.
    #[error("gRPC connection error: {0}")]
    Connect(#[from] tonic::transport::Error),

    /// Application-level gRPC status code returned by the remote server.
    #[error("gRPC status {code:?}: {message}")]
    Status {
        code: tonic::Code,
        message: String,
    },

    #[error("invalid gRPC metadata value: {0}")]
    InvalidMetadata(String),

    #[error("TLS configuration error: {0}")]
    Tls(String),
}

impl From<tonic::Status> for GrpcTransportError {
    fn from(s: tonic::Status) -> Self {
        Self::Status {
            code: s.code(),
            message: s.message().to_string(),
        }
    }
}

/// Maps the severity of a gRPC status code to a platform [`error::Severity`] so callers
/// can decide whether to page on-call.
///
/// - `UNAVAILABLE`, `DEADLINE_EXCEEDED`, `RESOURCE_EXHAUSTED` → `High` (usually transient)
/// - `INTERNAL`, `DATA_LOSS`, `UNKNOWN` → `Critical`
/// - `PERMISSION_DENIED`, `UNAUTHENTICATED` → `Medium`
/// - everything else → `Low`
pub fn grpc_severity(code: tonic::Code) -> error::Severity {
    use error::Severity;
    use tonic::Code;
    match code {
        Code::Internal | Code::DataLoss | Code::Unknown => Severity::Critical,
        Code::Unavailable | Code::DeadlineExceeded | Code::ResourceExhausted => Severity::High,
        Code::PermissionDenied | Code::Unauthenticated => Severity::Medium,
        _ => Severity::Low,
    }
}

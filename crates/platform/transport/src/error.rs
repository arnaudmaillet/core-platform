use std::time::Duration;

use thiserror::Error;

use crate::grpc::error::GrpcTransportError;
use crate::kafka::error::KafkaTransportError;

#[derive(Debug, Error)]
pub enum CodecError {
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("protobuf encode error: {0}")]
    ProtobufEncode(#[from] prost::EncodeError),

    #[error("protobuf decode error: {0}")]
    ProtobufDecode(#[from] prost::DecodeError),
}

/// Top-level transport error.
///
/// Every public function in this crate that can fail returns `Result<_, TransportError>`.
/// Sub-domain errors (`GrpcTransportError`, `KafkaTransportError`, `CodecError`) are kept
/// separate so callers can match precisely; the `#[from]` derives give ergonomic `?` usage.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("gRPC error: {0}")]
    Grpc(#[from] GrpcTransportError),

    #[error("Kafka error: {0}")]
    Kafka(#[from] KafkaTransportError),

    #[error("codec error: {0}")]
    Codec(#[from] CodecError),

    #[error("circuit breaker is open — request rejected")]
    CircuitOpen,

    #[error("request timed out after {0:?}")]
    Timeout(Duration),

    #[error("all {0} retry attempts exhausted")]
    MaxRetriesExhausted(u32),
}

impl TransportError {
    /// Flattens `ResilienceError<tonic::transport::Error>` (produced by circuit-breaker or
    /// timeout layers wrapping the raw channel) into `TransportError`.
    pub fn from_resilience_connect(
        e: resilience::error::ResilienceError<tonic::transport::Error>,
    ) -> Self {
        use resilience::error::ResilienceError;
        match e {
            ResilienceError::CircuitOpen => Self::CircuitOpen,
            ResilienceError::Timeout(d) => Self::Timeout(d),
            ResilienceError::MaxRetriesExhausted(n) => Self::MaxRetriesExhausted(n),
            ResilienceError::Inner(e) => Self::Grpc(GrpcTransportError::Connect(e)),
        }
    }

    /// Flattens `ResilienceError<TransportError>` produced by a resilience layer that already
    /// has a previously-mapped `TransportError` as its inner error type.
    pub fn from_resilience(e: resilience::error::ResilienceError<TransportError>) -> Self {
        use resilience::error::ResilienceError;
        match e {
            ResilienceError::CircuitOpen => Self::CircuitOpen,
            ResilienceError::Timeout(d) => Self::Timeout(d),
            ResilienceError::MaxRetriesExhausted(n) => Self::MaxRetriesExhausted(n),
            ResilienceError::Inner(t) => t,
        }
    }
}

/// Convenience `From` impl so `tonic::transport::Error` can be propagated via `?` in
/// functions that return `Result<_, TransportError>`.
impl From<tonic::transport::Error> for TransportError {
    fn from(e: tonic::transport::Error) -> Self {
        Self::Grpc(GrpcTransportError::Connect(e))
    }
}

/// Convenience `From` impl so `tonic::Status` can be propagated via `?`.
impl From<tonic::Status> for TransportError {
    fn from(s: tonic::Status) -> Self {
        Self::Grpc(GrpcTransportError::from(s))
    }
}

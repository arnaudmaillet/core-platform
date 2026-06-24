use std::time::Duration;

use thiserror::Error;

/// Top-level error type for all resilience middleware failures.
///
/// Generic over `E` — the inner service's own error type — so callers retain
/// full type information and can pattern-match on the cause.
#[derive(Debug, Error)]
pub enum ResilienceError<E> {
    #[error("circuit breaker is open — service unavailable")]
    CircuitOpen,

    #[error("request timed out after {0:?}")]
    Timeout(Duration),

    #[error("max retry attempts ({0}) exhausted")]
    MaxRetriesExhausted(u32),

    #[error(transparent)]
    Inner(E),
}

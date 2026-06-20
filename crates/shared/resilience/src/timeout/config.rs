use std::time::Duration;

/// Configuration for the timeout middleware.
#[derive(Debug, Clone, Copy)]
pub struct TimeoutConfig {
    /// Maximum duration to wait for the inner service to respond.
    pub duration: Duration,
}

impl TimeoutConfig {
    pub fn new(duration: Duration) -> Self {
        Self { duration }
    }

    pub fn from_secs(secs: u64) -> Self {
        Self::new(Duration::from_secs(secs))
    }

    pub fn from_millis(ms: u64) -> Self {
        Self::new(Duration::from_millis(ms))
    }
}

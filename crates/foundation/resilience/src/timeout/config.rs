use std::time::Duration;

/// Configuration for the timeout middleware.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimeoutConfig {
    /// Maximum duration to wait for the inner service to respond.
    /// Serialized as a flat `duration_ms` integer.
    #[cfg_attr(
        feature = "serde",
        serde(rename = "duration_ms", with = "crate::serde_util::duration_millis")
    )]
    pub duration: Duration,
}

impl Default for TimeoutConfig {
    /// Conservative 10s default, matching the transport-layer RPC default.
    fn default() -> Self {
        Self::from_secs(10)
    }
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

use thiserror::Error;

/// Errors raised while loading, parsing, validating, or watching the infrastructure config.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read infrastructure config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse infrastructure config TOML: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("filesystem watch error: {0}")]
    Watch(#[from] notify::Error),

    /// A structurally-valid config that violates a semantic invariant. Reported
    /// *before* any live swap, so the running fleet keeps its previous values.
    #[error("invalid infrastructure config: {0}")]
    Validation(String),
}

impl ConfigError {
    /// Constructs a [`ConfigError::Validation`]. Public so external
    /// [`TelemetrySink`](crate::TelemetrySink) implementors (which live outside
    /// this crate) can surface an apply failure as a config rejection.
    pub fn validation(msg: impl Into<String>) -> Self {
        ConfigError::Validation(msg.into())
    }
}

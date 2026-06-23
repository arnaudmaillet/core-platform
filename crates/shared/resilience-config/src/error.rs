use thiserror::Error;

/// Errors raised while loading, parsing, validating, or watching the resilience config.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read resilience config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse resilience config TOML: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("filesystem watch error: {0}")]
    Watch(#[from] notify::Error),

    /// A structurally-valid config that violates a semantic invariant. Reported
    /// *before* any live swap, so the running fleet keeps its previous values.
    #[error("invalid resilience config: {0}")]
    Validation(String),
}

impl ConfigError {
    pub(crate) fn validation(msg: impl Into<String>) -> Self {
        ConfigError::Validation(msg.into())
    }
}

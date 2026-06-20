/// Controls the structured-log layer.
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// `RUST_LOG`-compatible filter string used when the `RUST_LOG` env var is
    /// absent.  Defaults to `"info"`.
    pub default_filter: String,
    /// Wire format for log records written to stdout.
    pub format: LogFormat,
    /// Whether to emit ANSI colour codes.  Forced to `false` in JSON mode.
    pub ansi: bool,
}

/// Stdout wire format.
#[derive(Debug, Clone, Default)]
pub enum LogFormat {
    /// Machine-readable JSON — use in all container / production environments.
    #[default]
    Json,
    /// Human-readable — use in local development.
    Pretty,
}

impl LogConfig {
    /// Reads `LOG_FORMAT` (`json` | `pretty`) and `LOG_FILTER` env vars.
    pub fn from_env() -> Self {
        let format = match std::env::var("LOG_FORMAT").as_deref() {
            Ok("pretty") => LogFormat::Pretty,
            _ => LogFormat::Json,
        };
        let ansi = matches!(format, LogFormat::Pretty);
        Self {
            default_filter: std::env::var("LOG_FILTER").unwrap_or_else(|_| "info".into()),
            format,
            ansi,
        }
    }
}

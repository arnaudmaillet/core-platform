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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn log_format_default_is_json() {
        assert!(matches!(LogFormat::default(), LogFormat::Json));
    }

    #[test]
    fn defaults_when_no_env_vars() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: single-threaded under the mutex lock above.
        unsafe {
            std::env::remove_var("LOG_FORMAT");
            std::env::remove_var("LOG_FILTER");
        }
        let cfg = LogConfig::from_env();
        assert!(matches!(cfg.format, LogFormat::Json));
        assert_eq!(cfg.default_filter, "info");
        assert!(!cfg.ansi);
    }

    #[test]
    fn pretty_format_from_env() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: single-threaded under the mutex lock above.
        unsafe {
            std::env::set_var("LOG_FORMAT", "pretty");
            std::env::remove_var("LOG_FILTER");
        }
        let cfg = LogConfig::from_env();
        assert!(matches!(cfg.format, LogFormat::Pretty));
        assert!(cfg.ansi);
        // SAFETY: cleanup under the same lock.
        unsafe { std::env::remove_var("LOG_FORMAT") };
    }

    #[test]
    fn unknown_format_falls_back_to_json() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: single-threaded under the mutex lock above.
        unsafe { std::env::set_var("LOG_FORMAT", "unknown") };
        let cfg = LogConfig::from_env();
        assert!(matches!(cfg.format, LogFormat::Json));
        assert!(!cfg.ansi);
        // SAFETY: cleanup.
        unsafe { std::env::remove_var("LOG_FORMAT") };
    }

    #[test]
    fn custom_log_filter() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: single-threaded under the mutex lock above.
        unsafe {
            std::env::remove_var("LOG_FORMAT");
            std::env::set_var("LOG_FILTER", "debug");
        }
        let cfg = LogConfig::from_env();
        assert_eq!(cfg.default_filter, "debug");
        // SAFETY: cleanup.
        unsafe { std::env::remove_var("LOG_FILTER") };
    }
}

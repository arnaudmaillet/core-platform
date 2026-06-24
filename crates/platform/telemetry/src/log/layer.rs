use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, Layer};

use super::config::{LogConfig, LogFormat};
use crate::error::TelemetryError;

/// Builds a non-blocking stdout logging layer.
///
/// The [`WorkerGuard`] in the return value keeps the background writer thread
/// alive.  It **must** be stored in [`crate::TelemetryGuard`] — dropping it
/// early silently discards buffered log records.
pub fn build_log_layer<S>(
    config: &LogConfig,
) -> Result<(Box<dyn Layer<S> + Send + Sync>, WorkerGuard), TelemetryError>
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    let (writer, guard) = tracing_appender::non_blocking(std::io::stdout());

    let layer: Box<dyn Layer<S> + Send + Sync> = match config.format {
        LogFormat::Json => Box::new(
            fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_writer(writer)
                .with_ansi(false),
        ),
        LogFormat::Pretty => Box::new(
            fmt::layer()
                .pretty()
                .with_writer(writer)
                .with_ansi(config.ansi),
        ),
    };

    Ok((layer, guard))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::Registry;

    #[test]
    fn json_layer_builds_without_error() {
        let cfg = LogConfig {
            default_filter: "info".into(),
            format: LogFormat::Json,
            ansi: false,
        };
        let (_layer, _guard) = build_log_layer::<Registry>(&cfg).unwrap();
    }

    #[test]
    fn pretty_layer_builds_without_error() {
        let cfg = LogConfig {
            default_filter: "info".into(),
            format: LogFormat::Pretty,
            ansi: true,
        };
        let (_layer, _guard) = build_log_layer::<Registry>(&cfg).unwrap();
    }
}

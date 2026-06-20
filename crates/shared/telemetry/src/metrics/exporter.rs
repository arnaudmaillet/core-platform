/// A handle to the Prometheus metric registry, shared cheaply via [`Arc`].
///
/// Obtained from [`crate::TelemetryGuard::prometheus_handle`].
/// Pass it to [`metrics_route`] to mount the scrape endpoint in your Axum router.
pub struct PrometheusHandle {
    #[cfg(feature = "prometheus-exporter")]
    pub(crate) registry: prometheus::Registry,
}

impl PrometheusHandle {
    /// Renders the current metric snapshot as a Prometheus text exposition
    /// (`text/plain; version=0.0.4`).
    #[cfg(feature = "prometheus-exporter")]
    pub fn render(&self) -> String {
        use prometheus::{Encoder, TextEncoder};
        let encoder = TextEncoder::new();
        let mut buf = Vec::new();
        encoder
            .encode(&self.registry.gather(), &mut buf)
            .expect("prometheus text encoding is infallible");
        String::from_utf8(buf).expect("prometheus output is valid UTF-8")
    }

    #[cfg(not(feature = "prometheus-exporter"))]
    pub fn render(&self) -> String {
        String::new()
    }
}

/// Returns an Axum handler closure that serves the Prometheus text scrape.
///
/// Mount it once before starting your HTTP server:
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use axum::Router;
/// use telemetry::metrics::exporter::{PrometheusHandle, metrics_route};
///
/// fn build_router(handle: Arc<PrometheusHandle>) -> Router {
///     Router::new().route("/metrics", axum::routing::get(metrics_route(handle)))
/// }
/// ```
#[cfg(feature = "prometheus-exporter")]
pub fn metrics_route(
    handle: std::sync::Arc<PrometheusHandle>,
) -> impl Fn() -> std::future::Ready<(
    [(axum::http::HeaderName, axum::http::HeaderValue); 1],
    String,
)> + Clone {
    move || {
        let body = handle.render();
        let header = (
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
        );
        std::future::ready(([header], body))
    }
}

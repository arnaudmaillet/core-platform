use opentelemetry::{
    trace::TracerProvider as TracerProviderTrait,
    KeyValue,
};
use opentelemetry_sdk::{
    runtime,
    trace::{BatchSpanProcessor, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions::resource as semcov;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::Layer;

use super::config::{SamplingStrategy, TraceConfig};
use super::dynamic_sampler::{DynamicSampler, SamplingHandle};
use crate::error::TelemetryError;

/// What [`build_trace_layer`] yields: the tracing layer, the owning provider (for the
/// guard's flush-on-drop), and the handle that hot-swaps the sampling ratio.
type TraceLayerBuild<S> =
    (Box<dyn tracing_subscriber::Layer<S> + Send + Sync>, TracerProvider, SamplingHandle);

/// Builds the OpenTelemetry tracing layer and the owning [`TracerProvider`].
///
/// The provider **must** be stored inside [`crate::TelemetryGuard`];
/// `provider.shutdown()` on drop flushes all buffered spans to the collector.
///
/// Pipeline:
/// ```text
/// SpanExporter (OTLP/gRPC or HTTP/proto)
///   └─ BatchSpanProcessor (Tokio async, non-blocking)
///       └─ TracerProvider
///           └─ Resource { service.name, service.version }
///           └─ DynamicSampler  ParentBased(TraceIdRatioBased(ratio)) — hot-swappable
/// ```
pub fn build_trace_layer<S>(
    config: &TraceConfig,
    service_name: &str,
    service_version: &str,
) -> Result<TraceLayerBuild<S>, TelemetryError>
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span> + Send + Sync,
{
    let exporter = super::exporter::build_otlp_exporter(config)?;

    let resource = Resource::new(vec![
        KeyValue::new(semcov::SERVICE_NAME, service_name.to_string()),
        KeyValue::new(semcov::SERVICE_VERSION, service_version.to_string()),
    ]);

    // Install a dynamic, parent-respecting sampler initialised from config; keep its handle
    // so the ratio can be hot-swapped (the SDK bakes the sampler into the provider at build).
    let sampler = DynamicSampler::new(ratio_from(&config.sampling)?);
    let sampling_handle = sampler.handle();

    let batch = BatchSpanProcessor::builder(exporter, runtime::Tokio).build();

    let provider = TracerProvider::builder()
        .with_span_processor(batch)
        .with_sampler(sampler)
        .with_resource(resource)
        .build();

    opentelemetry::global::set_tracer_provider(provider.clone());

    let tracer = provider.tracer(service_name.to_string());
    let layer: Box<dyn Layer<S> + Send + Sync> = Box::new(OpenTelemetryLayer::new(tracer));

    Ok((layer, provider, sampling_handle))
}

/// Resolves a boot [`SamplingStrategy`] to a ratio in `[0.0, 1.0]`. `AlwaysOn`/`AlwaysOff`
/// map to `1.0`/`0.0`; an out-of-range explicit ratio fails loud at boot.
fn ratio_from(strategy: &SamplingStrategy) -> Result<f64, TelemetryError> {
    match strategy {
        SamplingStrategy::AlwaysOn => Ok(1.0),
        SamplingStrategy::AlwaysOff => Ok(0.0),
        SamplingStrategy::TraceIdRatio(ratio) => {
            if !(0.0..=1.0).contains(ratio) {
                return Err(TelemetryError::InvalidSamplingRatio(*ratio));
            }
            Ok(*ratio)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::TelemetryError;

    #[test]
    fn always_on_maps_to_one() {
        assert_eq!(ratio_from(&SamplingStrategy::AlwaysOn).unwrap(), 1.0);
    }

    #[test]
    fn always_off_maps_to_zero() {
        assert_eq!(ratio_from(&SamplingStrategy::AlwaysOff).unwrap(), 0.0);
    }

    #[test]
    fn ratio_passes_through_in_range() {
        assert_eq!(ratio_from(&SamplingStrategy::TraceIdRatio(0.5)).unwrap(), 0.5);
        assert_eq!(ratio_from(&SamplingStrategy::TraceIdRatio(0.0)).unwrap(), 0.0);
        assert_eq!(ratio_from(&SamplingStrategy::TraceIdRatio(1.0)).unwrap(), 1.0);
    }

    #[test]
    fn negative_ratio_returns_invalid_sampling_error() {
        let err = ratio_from(&SamplingStrategy::TraceIdRatio(-0.1)).unwrap_err();
        assert!(matches!(err, TelemetryError::InvalidSamplingRatio(r) if (r - (-0.1)).abs() < f64::EPSILON));
    }

    #[test]
    fn ratio_above_one_returns_invalid_sampling_error() {
        let err = ratio_from(&SamplingStrategy::TraceIdRatio(1.5)).unwrap_err();
        assert!(matches!(err, TelemetryError::InvalidSamplingRatio(r) if (r - 1.5).abs() < f64::EPSILON));
    }
}

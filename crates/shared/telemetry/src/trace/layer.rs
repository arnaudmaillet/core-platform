use opentelemetry::{
    trace::TracerProvider as TracerProviderTrait,
    KeyValue,
};
use opentelemetry_sdk::{
    runtime,
    trace::{BatchSpanProcessor, Sampler, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions::resource as semcov;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::Layer;

use super::config::{SamplingStrategy, TraceConfig};
use crate::error::TelemetryError;

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
///           └─ Sampler  { AlwaysOn | AlwaysOff | TraceIdRatio }
/// ```
pub fn build_trace_layer<S>(
    config: &TraceConfig,
    service_name: &str,
    service_version: &str,
) -> Result<(Box<dyn Layer<S> + Send + Sync>, TracerProvider), TelemetryError>
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span> + Send + Sync,
{
    let exporter = super::exporter::build_otlp_exporter(config)?;

    let resource = Resource::new(vec![
        KeyValue::new(semcov::SERVICE_NAME, service_name.to_string()),
        KeyValue::new(semcov::SERVICE_VERSION, service_version.to_string()),
    ]);

    let sampler = sampler_from(&config.sampling)?;

    let batch = BatchSpanProcessor::builder(exporter, runtime::Tokio).build();

    let provider = TracerProvider::builder()
        .with_span_processor(batch)
        .with_sampler(sampler)
        .with_resource(resource)
        .build();

    opentelemetry::global::set_tracer_provider(provider.clone());

    let tracer = provider.tracer(service_name.to_string());
    let layer: Box<dyn Layer<S> + Send + Sync> = Box::new(OpenTelemetryLayer::new(tracer));

    Ok((layer, provider))
}

fn sampler_from(strategy: &SamplingStrategy) -> Result<Sampler, TelemetryError> {
    match strategy {
        SamplingStrategy::AlwaysOn => Ok(Sampler::AlwaysOn),
        SamplingStrategy::AlwaysOff => Ok(Sampler::AlwaysOff),
        SamplingStrategy::TraceIdRatio(ratio) => {
            if !(0.0..=1.0).contains(ratio) {
                return Err(TelemetryError::InvalidSamplingRatio(*ratio));
            }
            Ok(Sampler::TraceIdRatioBased(*ratio))
        }
    }
}

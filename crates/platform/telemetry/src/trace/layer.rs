use opentelemetry::{trace::TracerProvider as TracerProviderTrait, KeyValue};
use opentelemetry_sdk::{
    runtime,
    trace::{BatchSpanProcessor, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions::resource as semcov;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::Layer;

use super::config::TraceConfig;
use super::dynamic_sampler::{sampler_for, DynamicSampler};
use crate::error::TelemetryError;

/// Builds the OpenTelemetry tracing layer, the owning [`TracerProvider`], and the
/// [`DynamicSampler`] driving its sampling decisions.
///
/// The provider **must** be stored inside [`crate::TelemetryGuard`];
/// `provider.shutdown()` on drop flushes all buffered spans to the collector. The
/// returned [`DynamicSampler`] is handed to [`crate::TelemetryControl`] so sampling
/// can be retuned at runtime.
///
/// Pipeline:
/// ```text
/// SpanExporter (OTLP/gRPC or HTTP/proto)
///   └─ BatchSpanProcessor (Tokio async, non-blocking)
///       └─ TracerProvider
///           └─ Resource { service.name, service.version }
///           └─ DynamicSampler  → swappable { AlwaysOn | AlwaysOff | ParentBased(ratio) }
/// ```
/// Everything a subscriber needs from the trace plane: the erased layer, the
/// provider (kept for shutdown flush), and the sampler control handle.
pub type BuiltTraceLayer<S> = (Box<dyn Layer<S> + Send + Sync>, TracerProvider, DynamicSampler);

pub fn build_trace_layer<S>(
    config: &TraceConfig,
    service_name: &str,
    service_version: &str,
) -> Result<BuiltTraceLayer<S>, TelemetryError>
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span> + Send + Sync,
{
    let exporter = super::exporter::build_otlp_exporter(config)?;

    let resource = Resource::new(vec![
        KeyValue::new(semcov::SERVICE_NAME, service_name.to_string()),
        KeyValue::new(semcov::SERVICE_VERSION, service_version.to_string()),
    ]);

    let sampler = DynamicSampler::new(sampler_for(&config.sampling)?);

    let batch = BatchSpanProcessor::builder(exporter, runtime::Tokio).build();

    let provider = TracerProvider::builder()
        .with_span_processor(batch)
        .with_sampler(sampler.clone())
        .with_resource(resource)
        .build();

    opentelemetry::global::set_tracer_provider(provider.clone());

    let tracer = provider.tracer(service_name.to_string());
    let layer: Box<dyn Layer<S> + Send + Sync> = Box::new(OpenTelemetryLayer::new(tracer));

    Ok((layer, provider, sampler))
}

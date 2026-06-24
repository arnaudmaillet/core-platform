use opentelemetry::{
    global,
    propagation::{Extractor, Injector},
    Context,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Marker supertrait that combines OTel's [`Injector`] and [`Extractor`] into a single
/// bound. All carrier types in this module (`GrpcHeaderInjector`, `KafkaHeaderInjector`,
/// etc.) implicitly satisfy it through their concrete impls.
pub trait MetadataCarrier: Injector + Extractor {}

impl<T: Injector + Extractor> MetadataCarrier for T {}

/// Injects the current `tracing` span's OpenTelemetry context into `carrier` using
/// the globally-registered text-map propagator (W3C TraceContext by default when
/// `telemetry::init()` has been called).
///
/// Call this before dispatching any outbound request — gRPC or Kafka.
pub fn inject_context<C: Injector>(carrier: &mut C) {
    let cx = tracing::Span::current().context();
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&cx, carrier);
    });
}

/// Extracts an OpenTelemetry [`Context`] from `carrier` using the globally-registered
/// text-map propagator. Returns a root context when no trace headers are present.
///
/// Call this on every inbound request — gRPC or Kafka — and call
/// [`tracing_opentelemetry::OpenTelemetrySpanExt::set_parent`] on the current span.
pub fn extract_context<C: Extractor>(carrier: &C) -> Context {
    global::get_text_map_propagator(|propagator| propagator.extract(carrier))
}

/// Sets `cx` as the parent of `span`, wiring the remote trace into the local
/// `tracing` hierarchy.
///
/// This is a thin convenience wrapper so callers do not need to import
/// `tracing_opentelemetry` directly.
pub fn set_parent(span: &tracing::Span, cx: Context) {
    span.set_parent(cx);
}

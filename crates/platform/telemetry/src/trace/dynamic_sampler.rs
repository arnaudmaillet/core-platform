//! A trace sampler whose decision policy can be swapped at runtime.
//!
//! The OpenTelemetry SDK fixes the sampler when the `TracerProvider` is built, so
//! changing sampling normally means a redeploy. [`DynamicSampler`] wraps an
//! [`ArcSwap`] over the concrete [`Sampler`] and delegates each decision to the
//! current one, so [`TelemetryControl::set_sampling`](crate::TelemetryControl::set_sampling)
//! can retune sampling live (e.g. drop to 1% during an incident, or raise it for
//! a debug window) with no restart.

use std::sync::Arc;

use arc_swap::ArcSwap;
use opentelemetry::trace::{Link, SamplingResult, SpanKind, TraceId};
use opentelemetry::{Context, KeyValue};
use opentelemetry_sdk::trace::{Sampler, ShouldSample};

use crate::error::TelemetryError;
use crate::trace::config::SamplingStrategy;

/// A [`ShouldSample`] that delegates to a hot-swappable inner [`Sampler`].
///
/// Cloning shares the same swap cell (it is `Arc`-backed), so the copy held by
/// the `TracerProvider` and the copy held by [`TelemetryControl`](crate::TelemetryControl)
/// see each other's updates.
#[derive(Clone)]
pub struct DynamicSampler {
    inner: Arc<ArcSwap<Sampler>>,
}

impl DynamicSampler {
    /// Creates a sampler initialised to `initial`.
    pub fn new(initial: Sampler) -> Self {
        Self { inner: Arc::new(ArcSwap::from_pointee(initial)) }
    }

    /// Atomically replaces the active sampler. Lock-free; in-flight decisions
    /// finish against whichever sampler they loaded.
    pub fn set(&self, sampler: Sampler) {
        self.inner.store(Arc::new(sampler));
    }
}

impl std::fmt::Debug for DynamicSampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DynamicSampler")
    }
}

impl ShouldSample for DynamicSampler {
    fn should_sample(
        &self,
        parent_context: Option<&Context>,
        trace_id: TraceId,
        name: &str,
        span_kind: &SpanKind,
        attributes: &[KeyValue],
        links: &[Link],
    ) -> SamplingResult {
        let current = self.inner.load();
        current.should_sample(parent_context, trace_id, name, span_kind, attributes, links)
    }
}

/// Resolves a [`SamplingStrategy`] into a concrete [`Sampler`].
///
/// Ratio sampling is wrapped in [`Sampler::ParentBased`] so a span inherits its
/// parent's decision when there is one (keeping distributed traces whole) and
/// only falls back to the probability for roots. `AlwaysOn`/`AlwaysOff` map
/// directly — they are deterministic regardless of parent.
pub fn sampler_for(strategy: &SamplingStrategy) -> Result<Sampler, TelemetryError> {
    match strategy {
        SamplingStrategy::AlwaysOn => Ok(Sampler::AlwaysOn),
        SamplingStrategy::AlwaysOff => Ok(Sampler::AlwaysOff),
        SamplingStrategy::TraceIdRatio(ratio) => {
            if !(0.0..=1.0).contains(ratio) {
                return Err(TelemetryError::InvalidSamplingRatio(*ratio));
            }
            Ok(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(*ratio))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_on_off_resolve() {
        assert!(matches!(sampler_for(&SamplingStrategy::AlwaysOn).unwrap(), Sampler::AlwaysOn));
        assert!(matches!(sampler_for(&SamplingStrategy::AlwaysOff).unwrap(), Sampler::AlwaysOff));
    }

    #[test]
    fn ratio_is_parent_based() {
        assert!(matches!(
            sampler_for(&SamplingStrategy::TraceIdRatio(0.25)).unwrap(),
            Sampler::ParentBased(_)
        ));
    }

    #[test]
    fn ratio_boundaries_valid() {
        sampler_for(&SamplingStrategy::TraceIdRatio(0.0)).unwrap();
        sampler_for(&SamplingStrategy::TraceIdRatio(1.0)).unwrap();
    }

    #[test]
    fn out_of_range_ratio_rejected() {
        assert!(matches!(
            sampler_for(&SamplingStrategy::TraceIdRatio(-0.1)).unwrap_err(),
            TelemetryError::InvalidSamplingRatio(_)
        ));
        assert!(matches!(
            sampler_for(&SamplingStrategy::TraceIdRatio(1.5)).unwrap_err(),
            TelemetryError::InvalidSamplingRatio(_)
        ));
    }

    #[test]
    fn set_swaps_active_sampler() {
        let s = DynamicSampler::new(Sampler::AlwaysOff);
        s.set(Sampler::AlwaysOn);
        // A clone observes the swap (shared cell).
        let clone = s.clone();
        clone.set(Sampler::AlwaysOff);
    }
}

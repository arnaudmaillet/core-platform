//! Runtime hot-swap of the trace-sampling ratio.
//!
//! The OTel SDK consumes the sampler **by value** at `TracerProvider` build, so it can't be
//! replaced afterward. We install a [`DynamicSampler`] once that reads the live sampler from
//! an [`ArcSwap`] on each span — the provider never changes; the ratio behind it does.
//!
//! The stored value is the fully-built `ParentBased(TraceIdRatioBased(ratio))`, so the hot
//! path is a single lock-free load + delegate with **no per-span allocation**; the `Box` is
//! rebuilt only when the ratio changes (rare). `ParentBased` ensures a child span respects
//! the upstream sampling decision — traces stay whole across service hops.

use std::sync::Arc;

use arc_swap::ArcSwap;
use opentelemetry::trace::{Link, SamplingResult, SpanKind, TraceId};
use opentelemetry::{Context, KeyValue};
use opentelemetry_sdk::trace::{Sampler, ShouldSample};

/// Builds the parent-respecting, ratio-based sampler for `ratio` (clamped to `[0.0, 1.0]`).
fn build(ratio: f64) -> Sampler {
    Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(ratio.clamp(0.0, 1.0))))
}

/// A [`ShouldSample`] whose effective ratio is hot-swappable. Install it on the
/// `TracerProvider`; reconfigure it through the [`SamplingHandle`] it hands out.
#[derive(Debug, Clone)]
pub struct DynamicSampler {
    inner: Arc<ArcSwap<Sampler>>,
}

impl DynamicSampler {
    /// Creates a sampler starting at `ratio` (`0.0` = off, `1.0` = all).
    pub fn new(ratio: f64) -> Self {
        Self { inner: Arc::new(ArcSwap::from_pointee(build(ratio))) }
    }

    /// A cloneable handle that swaps this sampler's ratio at runtime. All clones — and the
    /// installed sampler — share the same `ArcSwap`, so a swap reconfigures the live provider.
    pub fn handle(&self) -> SamplingHandle {
        SamplingHandle { inner: Arc::clone(&self.inner) }
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
        self.inner
            .load()
            .should_sample(parent_context, trace_id, name, span_kind, attributes, links)
    }
}

/// Hot-swaps the live trace-sampling ratio.
#[derive(Clone)]
pub struct SamplingHandle {
    inner: Arc<ArcSwap<Sampler>>,
}

impl SamplingHandle {
    /// Sets the head-based ratio (clamped to `[0.0, 1.0]`). Lock-free; observed by the next
    /// root span. `ParentBased` is preserved, so child spans keep following their parent.
    pub fn set(&self, ratio: f64) {
        self.inner.store(Arc::new(build(ratio)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::{SamplingDecision, TraceId};

    fn decide(sampler: &DynamicSampler, tid: u128) -> SamplingDecision {
        sampler
            .should_sample(None, TraceId::from_bytes(tid.to_be_bytes()), "span", &SpanKind::Internal, &[], &[])
            .decision
    }

    #[test]
    fn ratio_zero_drops_all_root_spans() {
        let s = DynamicSampler::new(1.0);
        s.handle().set(0.0);
        for tid in 1..50u128 {
            assert_eq!(decide(&s, tid), SamplingDecision::Drop);
        }
    }

    #[test]
    fn ratio_one_samples_all_root_spans() {
        let s = DynamicSampler::new(0.0);
        s.handle().set(1.0);
        for tid in 1..50u128 {
            assert_eq!(decide(&s, tid), SamplingDecision::RecordAndSample);
        }
    }

    #[test]
    fn out_of_range_is_clamped() {
        let s = DynamicSampler::new(0.5);
        s.handle().set(9.9); // clamps to 1.0
        assert_eq!(decide(&s, 42), SamplingDecision::RecordAndSample);
        s.handle().set(-3.0); // clamps to 0.0
        assert_eq!(decide(&s, 42), SamplingDecision::Drop);
    }
}

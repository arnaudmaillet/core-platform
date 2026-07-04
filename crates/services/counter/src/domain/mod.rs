//! The pure domain layer for counter-analytics — the aggregation model and the
//! windowed fold that turns a firehose of observations into flushable deltas.
//!
//! The centre of gravity is the [`aggregator`]: a clock-injected, I/O-free
//! transform that collapses N [`observation::Observation`]s into one
//! [`aggregator::WindowDelta`] per `(entity, metric, window)`. Everything here is
//! deterministic and unit-testable without containers, a broker, or a wall clock.
//!
//! The probabilistic structures themselves (HyperLogLog for unique cardinality,
//! Count-Min Sketch for trending) live in the infrastructure tier — the domain's
//! job is only to *classify* which metric uses which strategy
//! ([`value_object::Metric::aggregation`]) and to carry the inputs/outputs
//! ([`value_object::MemberId`] in, [`read::Cardinality`] out). That keeps the
//! domain free of any specific estimator implementation.

pub mod aggregator;
pub mod observation;
pub mod query;
pub mod read;
pub mod value_object;

pub use aggregator::{WindowAggregator, WindowDelta, WindowKey};
pub use observation::Observation;
pub use query::{
    BatchGetQuery, BatchReadout, TimeGranularity, TimeSeriesBucket, TimeSeriesQuery, TrendingItem,
    TrendingQuery, TrendingScope,
};
pub use read::{Cardinality, CountSnapshot, CounterValue};
pub use value_object::{
    Aggregation, EntityId, EntityKind, EntityRef, MemberId, Metric, MetricKind, PopularityScore,
    PopularityWeights, WindowId, WindowSize,
};

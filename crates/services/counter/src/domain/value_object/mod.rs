//! Pure value objects for the counter aggregation model. No I/O, no clock reads
//! (time is injected as `DateTime<Utc>` parameters), no store awareness.

pub mod entity;
pub mod member;
pub mod metric;
pub mod popularity;
pub mod window;

pub use entity::{EntityId, EntityKind, EntityRef};
pub use member::MemberId;
pub use metric::{Aggregation, Metric, MetricKind};
pub use popularity::{PopularityScore, PopularityWeights};
pub use window::{WindowId, WindowSize};

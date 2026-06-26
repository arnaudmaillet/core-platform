use async_trait::async_trait;

use crate::application::event::DeliverableEvent;
use crate::error::RealtimeError;

/// The dispatcher's upstream feed: the stream of already-decoded
/// [`DeliverableEvent`]s the fan-out loop consumes.
///
/// In production (Phase 4/5) this is backed by the Kafka `run_consumer` pipeline
/// over the upstream topics (`chat` messages, `notification.v1.events`,
/// `counter.v1.popularity`, `post.v1.events`) plus the decode layer that turns a
/// raw record into a `DeliverableEvent`. An in-memory fake backs the unit tests
/// of the fan-out loop ([`run_dispatch`](crate::application::run_dispatch)).
///
/// `next_event` yields `None` when the feed is drained (shutdown / a finite test
/// fixture). A decode/transport fault surfaces as the corresponding `RTM-8xxx` /
/// `RTM-9xxx` error so the loop can apply the runtime's retry/DLQ policy.
#[async_trait]
pub trait EventSource: Send + Sync + 'static {
    async fn next_event(&self) -> Result<Option<DeliverableEvent>, RealtimeError>;
}

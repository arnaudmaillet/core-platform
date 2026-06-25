//! Event-publication adapters for the `moderation.v1.events` topic.
//!
//! Every domain event is keyed by `actor_id` so an actor's moderation events keep
//! per-partition order — a reversal can never be delivered ahead of the
//! application it reverses. The `event_type` header carries the dotted routing key.

pub mod fanout_event_publisher;
pub mod kafka_event_publisher;
pub mod log_event_publisher;

pub use fanout_event_publisher::FanoutEventPublisher;
pub use kafka_event_publisher::KafkaEventPublisher;
pub use log_event_publisher::LogEventPublisher;

/// The single Kafka topic every moderation domain event is published to.
pub const TOPIC_MODERATION_EVENTS: &str = "moderation.v1.events";

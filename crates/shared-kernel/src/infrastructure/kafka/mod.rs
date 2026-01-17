// crates/shared-kernel/src/infrastructure/kafka/mod.rs

mod kafka_message_producer;
mod kafka_message_consumer;

pub use kafka_message_producer::KafkaMessageProducer;
pub use kafka_message_consumer::KafkaMessageConsumer;
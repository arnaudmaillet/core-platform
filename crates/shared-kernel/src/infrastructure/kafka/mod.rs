// crates/shared-kernel/src/infrastructure/kafka/mod.rs

mod kafka_message_consumer;
mod kafka_message_producer;

pub use kafka_message_consumer::KafkaMessageConsumer;
pub use kafka_message_producer::KafkaMessageProducer;

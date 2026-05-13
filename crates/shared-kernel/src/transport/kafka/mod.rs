#[cfg(feature = "kafka")]
mod consumer;
#[cfg(feature = "kafka")]
mod producer;

#[cfg(feature = "kafka")]
pub use consumer::KafkaEventConsumer;
#[cfg(feature = "kafka")]
pub use producer::KafkaEventProducer;

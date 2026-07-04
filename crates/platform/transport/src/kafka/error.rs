use thiserror::Error;

#[derive(Debug, Error)]
pub enum KafkaTransportError {
    #[error("Kafka producer error: {0}")]
    Producer(rdkafka::error::KafkaError),

    #[error("Kafka consumer error: {0}")]
    Consumer(rdkafka::error::KafkaError),

    #[error("Kafka client configuration error: {0}")]
    Config(String),

    #[error("message payload is empty")]
    EmptyPayload,

    #[error("invalid header value — must be valid UTF-8: {0}")]
    InvalidHeader(String),

    #[error("topic subscription error: {0}")]
    Subscribe(rdkafka::error::KafkaError),
}

impl From<rdkafka::error::KafkaError> for KafkaTransportError {
    fn from(e: rdkafka::error::KafkaError) -> Self {
        Self::Producer(e)
    }
}

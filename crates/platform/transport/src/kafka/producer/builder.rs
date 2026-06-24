use rdkafka::producer::FutureProducer;

use crate::{
    error::TransportError,
    kafka::{
        config::producer::ProducerConfig,
        error::KafkaTransportError,
        producer::handle::KafkaProducerHandle,
    },
};

/// Constructs a [`KafkaProducerHandle`] from a [`ProducerConfig`].
pub struct KafkaProducerBuilder {
    config: ProducerConfig,
}

impl KafkaProducerBuilder {
    pub fn new(config: ProducerConfig) -> Self {
        Self { config }
    }

    /// Builds the rdkafka [`FutureProducer`] and wraps it in [`KafkaProducerHandle`].
    ///
    /// Returns an error if the rdkafka client configuration is invalid (e.g. unreachable
    /// brokers are detected only at first produce time, not here).
    pub fn build(self) -> Result<KafkaProducerHandle, TransportError> {
        let producer: FutureProducer = self
            .config
            .to_rdkafka()
            .create()
            .map_err(|e| TransportError::Kafka(KafkaTransportError::Config(e.to_string())))?;

        Ok(KafkaProducerHandle::new(producer))
    }
}

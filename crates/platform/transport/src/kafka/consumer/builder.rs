use rdkafka::consumer::{Consumer, StreamConsumer};

use crate::{
    error::TransportError,
    kafka::{
        config::consumer::ConsumerConfig,
        consumer::handle::KafkaConsumerHandle,
        error::KafkaTransportError,
    },
};

/// Constructs a [`KafkaConsumerHandle`] from a [`ConsumerConfig`] and a list of topics.
pub struct KafkaConsumerBuilder {
    config: ConsumerConfig,
    topics: Vec<String>,
}

impl KafkaConsumerBuilder {
    pub fn new(config: ConsumerConfig) -> Self {
        Self {
            config,
            topics: Vec::new(),
        }
    }

    pub fn subscribe(mut self, topic: impl Into<String>) -> Self {
        self.topics.push(topic.into());
        self
    }

    pub fn subscribe_many(mut self, topics: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.topics.extend(topics.into_iter().map(Into::into));
        self
    }

    /// Creates the rdkafka [`StreamConsumer`], subscribes to the configured topics,
    /// and returns a [`KafkaConsumerHandle`].
    pub fn build(self) -> Result<KafkaConsumerHandle, TransportError> {
        let consumer: StreamConsumer = self
            .config
            .to_rdkafka()
            .create()
            .map_err(|e| TransportError::Kafka(KafkaTransportError::Config(e.to_string())))?;

        let topic_refs: Vec<&str> = self.topics.iter().map(String::as_str).collect();
        consumer
            .subscribe(&topic_refs)
            .map_err(|e| TransportError::Kafka(KafkaTransportError::Subscribe(e)))?;

        tracing::info!(
            topics = ?self.topics,
            group_id = %self.config.group_id,
            "Kafka consumer subscribed"
        );

        Ok(KafkaConsumerHandle::new(consumer))
    }
}

use crate::kafka::config::client::KafkaClientConfig;

/// Kafka producer settings, extending the shared [`KafkaClientConfig`].
#[derive(Debug, Clone)]
pub struct ProducerConfig {
    pub client: KafkaClientConfig,

    /// Required acknowledgements from brokers before considering a produce successful.
    /// `"all"` (default) = leader + all in-sync replicas — maximum durability.
    /// `"1"` = leader only.
    /// `"0"` = fire-and-forget.
    pub acks: String,

    /// Compression algorithm applied to batches: `"none"`, `"gzip"`, `"snappy"`,
    /// `"lz4"`, `"zstd"` (default: `"snappy"` — good throughput/CPU trade-off).
    pub compression: String,

    /// Milliseconds the producer waits before sending a batch to improve throughput.
    /// `0` disables batching (lowest latency). Default `5`.
    pub linger_ms: u32,

    /// Maximum number of in-flight produce requests per broker before blocking.
    /// Setting this to `1` ensures strict ordering when `acks = "all"`.
    pub max_in_flight: u32,
}

impl Default for ProducerConfig {
    fn default() -> Self {
        Self {
            client: KafkaClientConfig::default(),
            acks: "all".to_string(),
            compression: "snappy".to_string(),
            linger_ms: 5,
            max_in_flight: 5,
        }
    }
}

impl ProducerConfig {
    pub fn new(client: KafkaClientConfig) -> Self {
        Self {
            client,
            ..Default::default()
        }
    }

    pub(crate) fn to_rdkafka(&self) -> rdkafka::config::ClientConfig {
        let mut cfg = self.client.to_rdkafka();
        cfg.set("acks", &self.acks)
            .set("compression.type", &self.compression)
            .set("linger.ms", &self.linger_ms.to_string())
            .set("max.in.flight.requests.per.connection", &self.max_in_flight.to_string());
        cfg
    }
}

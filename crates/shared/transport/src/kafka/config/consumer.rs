use crate::kafka::config::client::KafkaClientConfig;

/// What to do when a consumer group has no committed offset for a partition.
#[derive(Debug, Clone, Default)]
pub enum AutoOffsetReset {
    /// Start from the earliest available message.
    Earliest,
    /// Start from the latest message (skip existing backlog). Default.
    #[default]
    Latest,
}

impl AutoOffsetReset {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Earliest => "earliest",
            Self::Latest => "latest",
        }
    }
}

/// Kafka consumer settings, extending the shared [`KafkaClientConfig`].
#[derive(Debug, Clone)]
pub struct ConsumerConfig {
    pub client: KafkaClientConfig,

    /// Consumer group ID — all consumers sharing the same ID form a group that load-balances
    /// partitions. Required.
    pub group_id: String,

    /// What offset to start from when there is no committed offset. Default: `Latest`.
    pub auto_offset_reset: AutoOffsetReset,

    /// Whether to commit offsets automatically. Setting `false` (default) lets the
    /// application control exactly-once or at-least-once semantics by calling
    /// [`rdkafka::consumer::Consumer::commit_message`] explicitly.
    pub enable_auto_commit: bool,

    /// Interval in milliseconds at which rdkafka sends keepalive heartbeats to the broker.
    /// Must be lower than the broker's `session.timeout.ms`. Default: `3000`.
    pub heartbeat_interval_ms: u32,

    /// Maximum time (ms) the broker waits before considering a consumer dead.
    /// Default: `10000`.
    pub session_timeout_ms: u32,
}

impl ConsumerConfig {
    pub fn new(client: KafkaClientConfig, group_id: impl Into<String>) -> Self {
        Self {
            client,
            group_id: group_id.into(),
            auto_offset_reset: AutoOffsetReset::Latest,
            enable_auto_commit: false,
            heartbeat_interval_ms: 3_000,
            session_timeout_ms: 10_000,
        }
    }

    pub(crate) fn to_rdkafka(&self) -> rdkafka::config::ClientConfig {
        let mut cfg = self.client.to_rdkafka();
        cfg.set("group.id", &self.group_id)
            .set("auto.offset.reset", self.auto_offset_reset.as_str())
            .set(
                "enable.auto.commit",
                if self.enable_auto_commit { "true" } else { "false" },
            )
            .set("heartbeat.interval.ms", &self.heartbeat_interval_ms.to_string())
            .set("session.timeout.ms", &self.session_timeout_ms.to_string());
        cfg
    }
}

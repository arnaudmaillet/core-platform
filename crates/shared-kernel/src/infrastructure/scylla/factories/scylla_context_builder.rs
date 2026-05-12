// crates/shared-kernel/src/infrastructure/scylla/factories/scylla_context_builder.rs
use crate::core::{Error, Result};
use crate::infrastructure::scylla::factories::ScyllaContext;
use std::time::Duration;

pub struct ScyllaContextBuilder {
    pub(crate) nodes: Vec<String>,
    pub(crate) keyspace: String,
    pub(crate) connect_timeout: Duration,
}

impl Default for ScyllaContextBuilder {
    fn default() -> Self {
        Self {
            nodes: vec!["127.0.0.1:9042".to_string()],
            keyspace: "profile".to_string(),
            connect_timeout: Duration::from_secs(5),
        }
    }
}

impl ScyllaContextBuilder {
    pub fn new() -> Result<Self> {
        let nodes_str = std::env::var("PROFILE_SCYLLA_NODES")
            .map_err(|_| Error::internal("PROFILE_SCYLLA_NODES must be set"))?;

        let keyspace = std::env::var("PROFILE_SCYLLA_KEYSPACE")
            .map_err(|_| Error::internal("PROFILE_SCYLLA_KEYSPACE must be set"))?;

        let timeout_secs = std::env::var("PROFILE_SCYLLA_CONNECT_TIMEOUT")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<u64>()
            .map_err(|_| Error::internal("Invalid PROFILE_SCYLLA_CONNECT_TIMEOUT"))?;

        Ok(Self {
            nodes: nodes_str.split(',').map(|s| s.trim().to_string()).collect(),
            keyspace,
            connect_timeout: Duration::from_secs(timeout_secs),
        })
    }

    pub fn with_nodes(mut self, nodes: Vec<String>) -> Self {
        self.nodes = nodes;
        self
    }

    pub fn with_keyspace(mut self, keyspace: impl Into<String>) -> Self {
        self.keyspace = keyspace.into();
        self
    }

    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.connect_timeout = duration;
        self
    }

    pub async fn build(self) -> Result<ScyllaContext> {
        ScyllaContext::restore(self).await
    }
}

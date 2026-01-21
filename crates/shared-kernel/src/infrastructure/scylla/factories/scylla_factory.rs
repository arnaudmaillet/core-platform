use std::sync::Arc;
use std::time::Duration;
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use crate::errors::{AppResult, AppError, ErrorCode};

pub struct ScyllaConfig {
    pub nodes: Vec<String>,
    pub keyspace: Option<String>,
    pub connect_timeout: Duration,
}

impl ScyllaConfig {
    pub fn from_env() -> AppResult<Self> {
        let nodes_str = std::env::var("SCYLLA_NODES")
            .unwrap_or_else(|_| "127.0.0.1:9042".to_string());

        let nodes = nodes_str.split(',').map(|s| s.to_string()).collect();

        Ok(Self {
            nodes,
            keyspace: std::env::var("SCYLLA_KEYSPACE").ok(),
            connect_timeout: Duration::from_secs(5),
        })
    }
}

pub async fn create_scylla_session(config: &ScyllaConfig) -> AppResult<Arc<Session>> {
    let builder = SessionBuilder::new()
        .known_nodes(&config.nodes);

    let session = builder
        .build()
        .await
        .map_err(|e| AppError::new(
            ErrorCode::InternalError,
            format!("Failed to connect to ScyllaDB: {}", e)
        ))?;

    if let Some(ks) = &config.keyspace {
        session.use_keyspace(ks, true)
            .await
            .map_err(|e| AppError::new(
                ErrorCode::InternalError,
                format!("Failed to switch to keyspace {}: {}", ks, e)
            ))?;
    }

    // On enveloppe la session dans un Arc ici
    Ok(Arc::new(session))
}
// crates/shared-kernel/src/infrastructure/scylla/factories/scylla_factory.rs

use crate::errors::{AppError, AppResult, ErrorCode};
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use std::sync::Arc;
use std::time::Duration;

pub struct ScyllaConfig {
    pub nodes: Vec<String>,
    pub keyspace: String,
    pub connect_timeout: Duration,
}

impl ScyllaConfig {
    pub fn from_env() -> AppResult<Self> {
        // Nodes et Keyspace restent obligatoires
        let nodes_str = std::env::var("PROFILE_SCYLLA_NODES")
            .map_err(|_| AppError::new(ErrorCode::InternalError, "PROFILE_SCYLLA_NODES must be set"))?;

        let keyspace = std::env::var("PROFILE_SCYLLA_KEYSPACE")
            .map_err(|_| AppError::new(ErrorCode::InternalError, "PROFILE_SCYLLA_KEYSPACE must be set"))?;

        // Timeout avec valeur par défaut (5s par défaut pour Scylla est standard)
        let connect_timeout_secs = std::env::var("PROFILE_SCYLLA_CONNECT_TIMEOUT")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<u64>()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Invalid PROFILE_SCYLLA_CONNECT_TIMEOUT"))?;

        Ok(Self {
            nodes: nodes_str.split(',').map(|s| s.trim().to_string()).collect(),
            keyspace,
            connect_timeout: Duration::from_secs(connect_timeout_secs),
        })
    }
}

pub async fn create_scylla_session(config: &ScyllaConfig) -> AppResult<Arc<Session>> {
    let session = SessionBuilder::new()
        .known_nodes(&config.nodes)
        .build()
        .await
        .map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;

    // Plus besoin de "if let Some(ks)", on utilise directement la String
    session.use_keyspace(&config.keyspace, true)
        .await
        .map_err(|e| AppError::new(ErrorCode::InternalError, format!("Failed to switch to keyspace {}: {}", config.keyspace, e)))?;

    Ok(Arc::new(session))
}
// crates/shared-kernel/src/infrastructure/scylla/factories/scylla_context.rs

use std::sync::Arc;
use std::time::Duration;
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use crate::errors::AppResult;
use crate::infrastructure::scylla::factories::{ScyllaConfig, ScyllaContextBuilder};

pub struct ScyllaContext {
    session: Arc<Session>,
    nodes: Vec<String>,
    keyspace: String,
    connect_timeout: Duration
}

impl ScyllaContext {
    pub fn builder() -> AppResult<ScyllaContextBuilder> {
        ScyllaContextBuilder::new()
    }

    pub fn builder_raw() -> ScyllaContextBuilder {
        ScyllaContextBuilder::default()
    }

    pub fn session(&self) -> Arc<Session> {
        self.session.clone()
    }
    
    pub fn nodes(&self) -> Vec<String> {
        self.nodes.clone()
    }

    pub fn keyspace(&self) -> String {
        self.keyspace.clone()
    }
    
    pub fn config(&self) -> ScyllaConfig {
        ScyllaConfig {
            connect_timeout: self.connect_timeout.clone(),
        }
    }

    pub(crate) async fn restore(builder: ScyllaContextBuilder) -> AppResult<Self> {
        let session = SessionBuilder::new()
            .known_nodes(&builder.nodes)
            .connection_timeout(builder.connect_timeout)
            .build()
            .await
            .map_err(|e| crate::errors::AppError::new(crate::errors::ErrorCode::InternalError, e.to_string()))?;

        session.use_keyspace(&builder.keyspace, true)
            .await
            .map_err(|e| crate::errors::AppError::new(crate::errors::ErrorCode::InternalError, e.to_string()))?;

        Ok(Self {
            session: Arc::new(session),
            nodes: builder.nodes,
            keyspace: builder.keyspace,
            connect_timeout: builder.connect_timeout
        })
    }
}
// crates/infra-test/src/orchestrator.rs

use anyhow::Result;
use async_trait::async_trait;

use crate::ScyllaOrchestrator;

#[async_trait]
pub trait DatabaseOrchestrator {
    async fn setup(&self) -> Result<()>;
    fn name(&self) -> &str;
}

#[async_trait::async_trait]
impl DatabaseOrchestrator for ScyllaOrchestrator {
    fn name(&self) -> &str {
        "ScyllaDB"
    }

    async fn setup(&self) -> Result<()> {
        self.ensure_schema_ready().await
    }
}

pub struct InfrastructureOrchestrator {
    orchestrators: Vec<Box<dyn DatabaseOrchestrator + Send + Sync>>,
}

impl InfrastructureOrchestrator {
    pub fn new() -> Self {
        Self {
            orchestrators: Vec::new(),
        }
    }

    pub fn add(&mut self, orchestrator: Box<dyn DatabaseOrchestrator + Send + Sync>) {
        self.orchestrators.push(orchestrator);
    }

    pub async fn run_all(&self) -> Result<()> {
        for orch in &self.orchestrators {
            tracing::info!("--- Initializing infrastructure: {} ---", orch.name());
            orch.setup().await?;
        }
        Ok(())
    }
}

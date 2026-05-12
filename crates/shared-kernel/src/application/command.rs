// crates/shared-kernel/src/application/command.rs

use crate::core::Result;
use crate::domain::{utils::RetryConfig, value_objects::RegionCode};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTarget<ID> {
    pub id: ID,
    pub region: RegionCode,
    pub expected_version: u64,
}

impl<ID> CommandTarget<ID> {
    pub fn new(id: ID, region: RegionCode, expected_version: u64) -> Self {
        Self {
            id,
            region,
            expected_version,
        }
    }
}

pub trait IdentifiableCommand {
    fn command_id(&self) -> Uuid;
    fn profile_id(&self) -> String;
    fn region(&self) -> String;
}

#[async_trait]
pub trait CommandHandler: Send + Sync {
    type Context: 'static + Send + Sync + Clone;
    type Command: 'static + Send + Sync + Clone;
    type Output: 'static + Send;

    async fn handle(&self, ctx: &Self::Context, cmd: Self::Command) -> Result<Self::Output>;

    fn retry_config(&self) -> RetryConfig {
        RetryConfig::default()
    }
}

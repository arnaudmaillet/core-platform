// crates/shared-kernel/src/application/command.rs

use crate::core::{Result, RetryConfig};
use async_trait::async_trait;

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

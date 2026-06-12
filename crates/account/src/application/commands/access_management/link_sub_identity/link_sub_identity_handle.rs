// crates/account/src/application/link_sub_identity/link_sub_identity_handler.rs

use crate::application::{
    commands::access_management::LinkSubIdentityCommand, context::AccountCommandCtx,
};
use async_trait::async_trait;
use shared_kernel::{
    command::CommandHandler,
    core::{Result, RetryConfig},
};

pub struct LinkSubIdentityHandler;

impl LinkSubIdentityHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for LinkSubIdentityHandler {
    type Context = AccountCommandCtx;
    type Command = LinkSubIdentityCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: LinkSubIdentityCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        account.link_sub_identity(cmd.sub_id)?;
        ctx.save(&mut account, cmd.command_id).await?;

        Ok(())
    }

    fn retry_config(&self) -> RetryConfig {
        RetryConfig {
            max_retries: 5,
            initial_backoff_ms: 50,
        }
    }
}

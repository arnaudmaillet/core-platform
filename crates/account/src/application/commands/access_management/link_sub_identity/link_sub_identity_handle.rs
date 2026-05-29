// crates/account/src/application/link_sub_identity/link_sub_identity_handler.rs

use crate::application::{
    commands::access_management::LinkSubIdentityCommand, context::AccountCommandContext,
};
use async_trait::async_trait;
use shared_kernel::{
    command::CommandHandler,
    core::{Result, RetryConfig, TransactionManager},
};
use std::marker::PhantomData;

pub struct LinkSubIdentityHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> LinkSubIdentityHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for LinkSubIdentityHandler<TM> {
    type Context = AccountCommandContext<TM>;
    type Command = LinkSubIdentityCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandContext<TM>,
        cmd: LinkSubIdentityCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        account.link_sub_identity(cmd.sub_id)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }

    fn retry_config(&self) -> RetryConfig {
        RetryConfig {
            max_retries: 5,
            initial_backoff_ms: 50,
        }
    }
}

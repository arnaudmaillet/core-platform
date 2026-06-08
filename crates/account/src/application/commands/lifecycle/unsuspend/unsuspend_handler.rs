// crates/account/src/application/unsuspend_account/unsuspend_account_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Result, TransactionManager};
use std::marker::PhantomData;
use tracing::info;

use crate::application::commands::lifecycle::UnsuspendCommand;
use crate::application::context::AccountCommandContext;

pub struct UnsuspendHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> UnsuspendHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for UnsuspendHandler<TM> {
    type Context = AccountCommandContext<TM>;
    type Command = UnsuspendCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandContext<TM>,
        cmd: UnsuspendCommand,
    ) -> Result<Self::Output> {
        if !ctx.ensure_executable(cmd.command_id, cmd.region).await? {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.unsuspend(cmd.reason)? {
            ctx.save(&mut account, Some(cmd.command_id)).await?;
        } else {
            info!(
                account_id = %account.account_id(),
                "no changes detected, skipping save"
            );
        }

        Ok(())
    }
}

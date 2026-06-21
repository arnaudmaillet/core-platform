// crates/account/src/application/suspend_account/suspend_account_use_case.rs
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Result, TransactionManager};
use std::marker::PhantomData;
use tracing::info;

use crate::application::commands::lifecycle::SuspendCommand;
use crate::application::context::AccountCommandCtx;

pub struct SuspendHandler;
impl SuspendHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for SuspendHandler {
    type Context = AccountCommandCtx;
    type Command = SuspendCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: SuspendCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.suspend(cmd.reason)? {
            ctx.save(&mut account, cmd.command_id).await?;
        } else {
            info!(
                account_id = %account.account_id(),
                "no changes detected, skipping save"
            );
        }

        Ok(())
    }
}

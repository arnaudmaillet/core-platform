// crates/account/src/application/unban_account/unban_account_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Result, TransactionManager};
use std::marker::PhantomData;
use tracing::info;

use crate::application::commands::moderation::UnbanCommand;
use crate::application::context::AccountCommandCtx;

pub struct UnbanHandler;

impl UnbanHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for UnbanHandler {
    type Context = AccountCommandCtx;
    type Command = UnbanCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountCommandCtx, cmd: UnbanCommand) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.unban(cmd.reason)? {
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

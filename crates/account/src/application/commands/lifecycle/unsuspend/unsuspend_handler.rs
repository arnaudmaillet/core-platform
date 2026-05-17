// crates/account/src/application/unsuspend_account/unsuspend_account_use_case.rs

use async_trait::async_trait;

use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::lifecycle::UnsuspendCommand;
use crate::application::context::AccountContext;

pub struct UnsuspendHandler;

#[async_trait]
impl CommandHandler for UnsuspendHandler {
    type Context = AccountContext;
    type Command = UnsuspendCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: UnsuspendCommand) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, &cmd.target.region)
            .await?
        {
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

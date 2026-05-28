// crates/account/src/application/suspend_account/suspend_account_use_case
use async_trait::async_trait;

use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::lifecycle::SuspendCommand;
use crate::application::context::AccountCommandContext;

pub struct SuspendHandler;

#[async_trait]
impl CommandHandler for SuspendHandler {
    type Context = AccountCommandContext;
    type Command = SuspendCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountCommandContext, cmd: SuspendCommand) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.suspend(cmd.reason)? {
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

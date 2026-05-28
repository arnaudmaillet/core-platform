// crates/account/src/application/unban_account/unban_account_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::moderation::UnbanCommand;
use crate::application::context::AccountCommandContext;

pub struct UnbanHandler;

#[async_trait]
impl CommandHandler for UnbanHandler {
    type Context = AccountCommandContext;
    type Command = UnbanCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountCommandContext, cmd: UnbanCommand) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.unban(cmd.reason)? {
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

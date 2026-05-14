// crates/account/src/application/ban_account/ban_account_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::moderation::BanCommand;
use crate::application::context::AccountContext;

pub struct BanHandler;

#[async_trait]
impl CommandHandler for BanHandler {
    type Context = AccountContext;
    type Command = BanCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: BanCommand) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;

        if account.ban(cmd.reason)? {
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

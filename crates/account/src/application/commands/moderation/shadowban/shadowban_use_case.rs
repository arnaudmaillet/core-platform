// crates/account/src/application/shadowban_account/shadowban_account_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::moderation::ShadowbanCommand;
use crate::application::context::AccountContext;

pub struct ShadowbanHandler;

#[async_trait]
impl CommandHandler for ShadowbanHandler {
    type Context = AccountContext;
    type Command = ShadowbanCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: ShadowbanCommand) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.shadowban(cmd.reason)? {
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

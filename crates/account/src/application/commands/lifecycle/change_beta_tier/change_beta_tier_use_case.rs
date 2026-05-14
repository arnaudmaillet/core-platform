// crates/account/src/application/set_beta_status/set_as_beta_account_use_case.rs
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::context::AccountContext;
use crate::commands::lifecycle::ChangeBetaTierCommand;

pub struct ChangeBetaTierHandler;

#[async_trait]
impl CommandHandler for ChangeBetaTierHandler {
    type Context = AccountContext;
    type Command = ChangeBetaTierCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountContext,
        cmd: ChangeBetaTierCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;

        if account.change_beta_tier(cmd.new_tier)? {
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

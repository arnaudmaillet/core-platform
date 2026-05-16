// crates/account/src/application/reactivate_account/reactivate_account_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::lifecycle::ActivateCommand;
use crate::application::context::AccountContext;

pub struct ActivateHandler;

#[async_trait]
impl CommandHandler for ActivateHandler {
    type Context = AccountContext;
    type Command = ActivateCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: ActivateCommand) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, &cmd.target.region)
            .await?
        {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;

        if account.activate()? {
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

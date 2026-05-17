// crates/account/src/application/change_email/change_email_use_case.rs
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::settings::ChangeEmailCommand;
use crate::application::context::AccountContext;

pub struct ChangeEmailHandler;

#[async_trait]
impl CommandHandler for ChangeEmailHandler {
    type Context = AccountContext;
    type Command = ChangeEmailCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: ChangeEmailCommand) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, &cmd.target.region)
            .await?
        {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.change_email(cmd.new_email)? {
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

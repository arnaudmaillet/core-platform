// crates/account/src/application/update_timezone/mod.rs
use crate::application::commands::settings::UpdateTimezoneCommand;
use crate::application::context::AccountContext;
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

pub struct UpdateTimezoneHandler;

#[async_trait]
impl CommandHandler for UpdateTimezoneHandler {
    type Context = AccountContext;
    type Command = UpdateTimezoneCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountContext,
        cmd: UpdateTimezoneCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, &cmd.target.region)
            .await?
        {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.update_timezone(cmd.new_timezone)? {
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

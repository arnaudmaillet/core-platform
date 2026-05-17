// crates/account/src/application/remove_push_token/remove_push_token_use_case.rs
use crate::application::commands::settings::RemovePushTokenCommand;
use crate::application::context::AccountContext;
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

pub struct RemovePushTokenHandler;

#[async_trait]
impl CommandHandler for RemovePushTokenHandler {
    type Context = AccountContext;
    type Command = RemovePushTokenCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountContext,
        cmd: RemovePushTokenCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, &cmd.target.region)
            .await?
        {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.remove_push_token(cmd.token)? {
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

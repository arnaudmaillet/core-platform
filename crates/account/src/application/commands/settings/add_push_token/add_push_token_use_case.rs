// crates/account/src/application/add_push_token/add_push_token_use_case.rs
use crate::application::commands::settings::AddPushTokenCommand;
use crate::application::context::AccountContext;
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

pub struct AddPushTokenHandler;

#[async_trait]
impl CommandHandler for AddPushTokenHandler {
    type Context = AccountContext;
    type Command = AddPushTokenCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: AddPushTokenCommand) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.add_push_token(cmd.token)? {
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

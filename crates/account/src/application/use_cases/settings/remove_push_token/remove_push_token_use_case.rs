// crates/account/src/application/remove_push_token/remove_push_token_use_case.rs
use crate::application::context::AccountContext;
use crate::application::use_cases::settings::RemovePushTokenCommand;
use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::core::Result;

pub struct RemovePushTokenHandler;

#[async_trait]
impl CommandHandler for RemovePushTokenHandler {
    type Context = AccountContext;
    type Command = RemovePushTokenCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: RemovePushTokenCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.remove_push_token(cmd.token)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}

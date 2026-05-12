// crates/account/src/application/add_push_token/add_push_token_use_case.rs
use crate::application::context::AccountContext;
use crate::application::use_cases::settings::AddPushTokenCommand;
use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::core::Result;

pub struct AddPushTokenHandler;

#[async_trait]
impl CommandHandler for AddPushTokenHandler {
    type Context = AccountContext;
    type Command = AddPushTokenCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: AddPushTokenCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.add_push_token(cmd.token)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}

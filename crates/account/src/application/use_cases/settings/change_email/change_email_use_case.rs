// crates/account/src/application/change_email/change_email_use_case.rs
use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::core::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::settings::ChangeEmailCommand;

pub struct ChangeEmailHandler;

#[async_trait]
impl CommandHandler for ChangeEmailHandler {
    type Context = AccountContext;
    type Command = ChangeEmailCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: ChangeEmailCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.change_email(cmd.new_email)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}

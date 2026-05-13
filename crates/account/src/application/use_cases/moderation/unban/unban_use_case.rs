// crates/account/src/application/unban_account/unban_account_use_case.rs

use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::core::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::moderation::UnbanCommand;

pub struct UnbanHandler;

#[async_trait]
impl CommandHandler for UnbanHandler {
    type Context = AccountContext;
    type Command = UnbanCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: UnbanCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.unban(cmd.reason)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}
// crates/account/src/application/suspend_account/suspend_account_use_case
use async_trait::async_trait;

use shared_kernel::application::CommandHandler;
use shared_kernel::core::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::lifecycle::SuspendCommand;

pub struct SuspendHandler;

#[async_trait]
impl CommandHandler for SuspendHandler {
    type Context = AccountContext;
    type Command = SuspendCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: SuspendCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.suspend(cmd.reason)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;
        Ok(())
    }
}

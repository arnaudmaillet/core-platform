// crates/account/src/application/unsuspend_account/unsuspend_account_use_case.rs

use async_trait::async_trait;

use shared_kernel::application::CommandHandler;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::lifecycle::unsuspend::UnsuspendCommand;

pub struct UnsuspendHandler;

#[async_trait]
impl CommandHandler for UnsuspendHandler {
    type Context = AccountContext;
    type Command = UnsuspendCommand;
    type Output = ();

        async fn handle(&self, ctx: &AccountContext, cmd: UnsuspendCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.unsuspend(cmd.reason)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;
        Ok(())
    }
}
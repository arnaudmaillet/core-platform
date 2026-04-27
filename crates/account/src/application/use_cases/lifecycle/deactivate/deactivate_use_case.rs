// crates/account/src/application/deactivate_account/deactivate_account_use_case.rs

use async_trait::async_trait;

use shared_kernel::application::CommandHandler;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::lifecycle::deactivate::DeactivateCommand;

pub struct DeactivateHandler;

#[async_trait]
impl CommandHandler for DeactivateHandler {
    type Context = AccountContext;
    type Command = DeactivateCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: DeactivateCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.deactivate(cmd.reason)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;
        Ok(())
    }
}

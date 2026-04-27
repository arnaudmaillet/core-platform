// crates/account/src/application/set_beta_status/set_as_beta_account_use_case.rs
use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::settings::set_as_beta::SetAsBetaCommand;

pub struct SetAsBetaHandler;

#[async_trait]
impl CommandHandler for SetAsBetaHandler {
    type Context = AccountContext;
    type Command = SetAsBetaCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: SetAsBetaCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.unban(cmd.reason)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}

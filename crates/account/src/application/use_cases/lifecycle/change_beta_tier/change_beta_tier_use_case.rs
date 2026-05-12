// crates/account/src/application/set_beta_status/set_as_beta_account_use_case.rs
use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::core::Result;

use crate::application::context::AccountContext;
use crate::use_cases::lifecycle::ChangeBetaTierCommand;

pub struct ChangeBetaTierHandler;

#[async_trait]
impl CommandHandler for ChangeBetaTierHandler {
    type Context = AccountContext;
    type Command = ChangeBetaTierCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountContext,
        cmd: ChangeBetaTierCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.change_beta_tier(cmd.new_tier)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}

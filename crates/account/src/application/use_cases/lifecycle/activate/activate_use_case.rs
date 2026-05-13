// crates/account/src/application/reactivate_account/reactivate_account_use_case.rs

use shared_kernel::application::CommandHandler;
use shared_kernel::core::Result;
use async_trait::async_trait;

use crate::application::context::AccountContext;
use crate::application::use_cases::lifecycle::ActivateCommand;

pub struct ActivateHandler;

#[async_trait]
impl CommandHandler for ActivateHandler {
    type Context = AccountContext;
    type Command = ActivateCommand;
    type Output = ();

        async fn handle(
        &self,
        ctx: &AccountContext,
        cmd: ActivateCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.activate()?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;
        Ok(())
    }
}
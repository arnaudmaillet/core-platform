// crates/account/src/application/use_cases/personal_management/change_region/change_region_handler.rs

use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::settings::ChangeRegionCommand;

pub struct ChangeRegionHandler;

#[async_trait]
impl CommandHandler for ChangeRegionHandler {
    type Context = AccountContext;
    type Command = ChangeRegionCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: ChangeRegionCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.change_region(cmd.new_region)?;

        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}

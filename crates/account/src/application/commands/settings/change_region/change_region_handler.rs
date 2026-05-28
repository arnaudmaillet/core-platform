// crates/account/src/application/use_cases/personal_management/change_region/change_region_handler.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;

use crate::application::commands::settings::ChangeRegionCommand;
use crate::application::context::AccountCommandContext;

pub struct ChangeRegionHandler;

#[async_trait]
impl CommandHandler for ChangeRegionHandler {
    type Context = AccountCommandContext;
    type Command = ChangeRegionCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountCommandContext, cmd: ChangeRegionCommand) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }

        let mut account = ctx.fetch_verified(&cmd.target).await?;

        account.change_region(cmd.new_region)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}

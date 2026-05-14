// crates/account/src/application/use_cases/personal_management/change_region/change_region_handler.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::settings::ChangeRegionCommand;
use crate::application::context::AccountContext;

pub struct ChangeRegionHandler;

#[async_trait]
impl CommandHandler for ChangeRegionHandler {
    type Context = AccountContext;
    type Command = ChangeRegionCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: ChangeRegionCommand) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.change_region(cmd.new_region)? {
            ctx.save(&mut account, Some(cmd.command_id)).await?;
        } else {
            info!(
                account_id = %account.account_id(),
                "no changes detected, skipping save"
            );
        }

        Ok(())
    }
}

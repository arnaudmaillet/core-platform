// crates/account/src/application/change_role/change_role_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::lifecycle::ChangeRoleCommand;
use crate::application::context::AccountContext;

pub struct ChangeRoleHandler;

#[async_trait]
impl CommandHandler for ChangeRoleHandler {
    type Context = AccountContext;
    type Command = ChangeRoleCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: ChangeRoleCommand) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.change_role(cmd.new_role, cmd.reason)? {
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

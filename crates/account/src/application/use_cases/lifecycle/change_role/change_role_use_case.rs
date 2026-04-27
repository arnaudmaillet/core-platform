// crates/account/src/application/change_role/change_role_use_case.rs

use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::lifecycle::change_role::ChangeRoleCommand;

pub struct ChangeRoleHandler;

#[async_trait]
impl CommandHandler for ChangeRoleHandler {
    type Context = AccountContext;
    type Command = ChangeRoleCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: ChangeRoleCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.change_role(cmd.new_role, cmd.reason)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;
        Ok(())
    }
}

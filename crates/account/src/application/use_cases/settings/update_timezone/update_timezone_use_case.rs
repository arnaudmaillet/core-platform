// crates/account/src/application/update_timezone/mod.rs
use async_trait::async_trait;
use crate::application::context::AccountContext;
use crate::application::use_cases::settings::update_timezone::update_timezone_command::UpdateTimezoneCommand;
use shared_kernel::application::CommandHandler;
use shared_kernel::errors::Result;

pub struct UpdateTimezoneHandler;

#[async_trait]
impl CommandHandler for UpdateTimezoneHandler {
    type Context = AccountContext;
    type Command = UpdateTimezoneCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: UpdateTimezoneCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.update_timezone(cmd.new_timezone)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}

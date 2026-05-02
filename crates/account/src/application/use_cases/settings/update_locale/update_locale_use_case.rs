// crates/account/src/application/update_locale/update_locale_use_case.rs
use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::settings::UpdateLocaleCommand;

pub struct UpdateLocaleHandler;

#[async_trait]
impl CommandHandler for UpdateLocaleHandler {
    type Context = AccountContext;
    type Command = UpdateLocaleCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: UpdateLocaleCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.update_locale(cmd.new_locale)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}

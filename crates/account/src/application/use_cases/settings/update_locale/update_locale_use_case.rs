// crates/account/src/application/update_locale/update_locale_use_case.rs
use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::settings::UpdateLocaleCommand;

pub struct UpdateLocaleHandler;

// crates/account/src/application/update_locale/update_locale_use_case.rs

#[async_trait]
impl CommandHandler for UpdateLocaleHandler {
    type Context = AccountContext;
    type Command = UpdateLocaleCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: UpdateLocaleCommand) -> Result<Self::Output> {
        let result = ctx.account().await;
        let mut account = result?;

        account.update_locale(cmd.new_locale)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}

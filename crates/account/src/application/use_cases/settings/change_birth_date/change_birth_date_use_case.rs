// crates/account/src/application/change_birth_date/change_birth_date_use_case.rs
use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::core::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::settings::ChangeBirthDateCommand;

pub struct ChangeBirthDateHandler;

#[async_trait]
impl CommandHandler for ChangeBirthDateHandler {
    type Context = AccountContext;
    type Command = ChangeBirthDateCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountContext,
        cmd: ChangeBirthDateCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.change_birth_date(cmd.new_birth_date)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}

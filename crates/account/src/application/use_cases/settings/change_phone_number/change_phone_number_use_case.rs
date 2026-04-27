// crates/account/src/application/change_email/change_phone_number_use_case.rs
use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::settings::change_phone_number::change_phone_number_command::ChangePhoneNumberCommand;

pub struct ChangePhoneNumberHandler;

#[async_trait]
impl CommandHandler for ChangePhoneNumberHandler {
    type Context = AccountContext;
    type Command = ChangePhoneNumberCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountContext,
        cmd: ChangePhoneNumberCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.change_phone(cmd.new_phone)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}

// crates/account/src/application/verify_phone_number/verify_phone_number_use_case.rs
use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::access_management::verify_phone_number::VerifyPhoneNumberCommand;

pub struct VerifyPhoneNumberHandler;

#[async_trait]
impl CommandHandler for VerifyPhoneNumberHandler {
    type Context = AccountContext;
    type Command = VerifyPhoneNumberCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountContext,
        cmd: VerifyPhoneNumberCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.verify_phone(cmd.token)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;
        Ok(())
    }
}

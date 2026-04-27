// crates/account/src/application/verify_email/verify_email_use_case.rs

use crate::application::{
    context::AccountContext, use_cases::access_management::verify_email::VerifyEmailCommand,
};
use async_trait::async_trait;
use shared_kernel::{application::CommandHandler, errors::Result};

pub struct VerifyEmailHandler;

#[async_trait]
impl CommandHandler for VerifyEmailHandler {
    type Context = AccountContext;
    type Command = VerifyEmailCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: VerifyEmailCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;

        account.verify_email_token(&cmd.token)?;
        ctx.save(&mut account).await?;

        Ok(())
    }
}

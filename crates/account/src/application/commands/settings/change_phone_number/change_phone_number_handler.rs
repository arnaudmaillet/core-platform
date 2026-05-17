// crates/account/src/application/change_email/change_phone_number_use_case.rs
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::settings::ChangePhoneNumberCommand;
use crate::application::context::AccountContext;

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
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.change_phone(cmd.new_phone)? {
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

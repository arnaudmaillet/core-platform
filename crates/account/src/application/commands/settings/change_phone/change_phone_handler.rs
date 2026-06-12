// crates/account/src/application/change_email/change_phone_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::settings::ChangePhoneCommand;
use crate::application::context::AccountCommandCtx;
use crate::domain::types::RegistrationIdentifier;

pub struct ChangePhoneNumberHandler;

impl ChangePhoneNumberHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for ChangePhoneNumberHandler {
    type Context = AccountCommandCtx;
    type Command = ChangePhoneCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: ChangePhoneCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        let account_id = account.account_id();

        let new_identifiers = RegistrationIdentifier::try_from_options(
            account.identity().email().cloned(),
            Some(cmd.new_phone.clone()),
        )?;

        ctx.global_registry()
            .update_identifiers(account_id, new_identifiers)
            .await?;

        if account.change_phone(cmd.new_phone)? {
            ctx.save(&mut account, cmd.command_id).await?;

            info!(
                account_id = %account_id,
                "Account phone updated successfully both globally and regionally"
            );
        } else {
            info!(
                account_id = %account_id,
                "No changes detected for phone, regional state skipped"
            );
        }

        Ok(())
    }
}

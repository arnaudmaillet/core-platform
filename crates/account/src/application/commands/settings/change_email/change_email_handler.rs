// crates/account/src/application/change_email/change_email_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Result, TransactionManager};
use std::marker::PhantomData;
use tracing::info;

use crate::application::commands::settings::ChangeEmailCommand;
use crate::application::context::AccountCommandCtx;
use crate::domain::types::RegistrationIdentifier;

pub struct ChangeEmailHandler;

impl ChangeEmailHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for ChangeEmailHandler {
    type Context = AccountCommandCtx;
    type Command = ChangeEmailCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: ChangeEmailCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        let account_id = account.account_id();
        let new_identifiers = RegistrationIdentifier::try_from_options(
            Some(cmd.new_email.clone()),
            account.identity().phone().cloned(),
        )?;

        ctx.global_registry()
            .update_identifiers(account_id, new_identifiers)
            .await?;

        if account.change_email(cmd.new_email)? {
            ctx.save(&mut account, cmd.command_id).await?;
            info!(
                account_id = %account_id,
                "Account email updated successfully both globally and regionally"
            );
        } else {
            info!(
                account_id = %account_id,
                "No changes detected for email, regional state skipped"
            );
        }

        Ok(())
    }
}

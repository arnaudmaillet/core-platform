// crates/account/src/application/change_email/change_phone_number_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Result, TransactionManager};
use std::marker::PhantomData;
use tracing::info;

use crate::application::commands::settings::ChangePhoneNumberCommand;
use crate::application::context::AccountCommandContext;
use crate::domain::types::RegistrationIdentifier;

pub struct ChangePhoneNumberHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> ChangePhoneNumberHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for ChangePhoneNumberHandler<TM> {
    type Context = AccountCommandContext<TM>;
    type Command = ChangePhoneNumberCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandContext<TM>,
        cmd: ChangePhoneNumberCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }

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
            ctx.save(&mut account, Some(cmd.command_id)).await?;
            info!(
                account_id = %account_id,
                "Account phone number updated successfully both globally and regionally"
            );
        } else {
            info!(
                account_id = %account_id,
                "No changes detected for phone number, regional state skipped"
            );
        }

        Ok(())
    }
}

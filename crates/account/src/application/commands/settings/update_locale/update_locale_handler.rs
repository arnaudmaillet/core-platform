// crates/account/src/application/update_locale/update_locale_use_case.rs
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Result, TransactionManager};
use std::marker::PhantomData;
use tracing::info;

use crate::application::commands::settings::UpdateLocaleCommand;
use crate::application::context::AccountCommandContext;

pub struct UpdateLocaleHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> UpdateLocaleHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for UpdateLocaleHandler<TM> {
    type Context = AccountCommandContext<TM>;
    type Command = UpdateLocaleCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandContext<TM>,
        cmd: UpdateLocaleCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.region)
            .await?
        {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;

        if account.update_locale(cmd.new_locale)? {
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

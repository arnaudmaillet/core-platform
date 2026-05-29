// crates/account/src/application/update_timezone/mod.rs
use crate::application::commands::settings::UpdateTimezoneCommand;
use crate::application::context::AccountCommandContext;
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Result, TransactionManager};
use std::marker::PhantomData;
use tracing::info;

pub struct UpdateTimezoneHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> UpdateTimezoneHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for UpdateTimezoneHandler<TM> {
    type Context = AccountCommandContext<TM>;
    type Command = UpdateTimezoneCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandContext<TM>,
        cmd: UpdateTimezoneCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.update_timezone(cmd.new_timezone)? {
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

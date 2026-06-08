// crates/account/src/application/set_beta_status/set_as_beta_account_use_case.rs
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Result, TransactionManager};
use std::marker::PhantomData;
use tracing::info;

use crate::application::context::AccountCommandContext;
use crate::commands::lifecycle::ChangeBetaTierCommand;

pub struct ChangeBetaTierHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> ChangeBetaTierHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for ChangeBetaTierHandler<TM> {
    type Context = AccountCommandContext<TM>;
    type Command = ChangeBetaTierCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandContext<TM>,
        cmd: ChangeBetaTierCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.region)
            .await?
        {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;

        if account.change_beta_tier(cmd.new_tier)? {
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

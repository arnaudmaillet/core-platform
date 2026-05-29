// crates/account/src/application/decrease_trust_score/decrease_trust_score_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Result, TransactionManager};
use std::marker::PhantomData;
use tracing::info;

use crate::application::commands::moderation::DecreaseTrustScoreCommand;
use crate::application::context::AccountCommandContext;

pub struct DecreaseTrustScoreHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> DecreaseTrustScoreHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for DecreaseTrustScoreHandler<TM> {
    type Context = AccountCommandContext<TM>;
    type Command = DecreaseTrustScoreCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandContext<TM>,
        cmd: DecreaseTrustScoreCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }

        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.penalize_trust(cmd.amount, cmd.reason)? {
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

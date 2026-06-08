// crates/account/src/application/increase_trust_score/increase_trust_score_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Result, TransactionManager};
use std::marker::PhantomData;
use tracing::info;

use crate::application::commands::moderation::IncreaseTrustScoreCommand;
use crate::application::context::AccountCommandContext;

pub struct IncreaseTrustScoreHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> IncreaseTrustScoreHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for IncreaseTrustScoreHandler<TM> {
    type Context = AccountCommandContext<TM>;
    type Command = IncreaseTrustScoreCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandContext<TM>,
        cmd: IncreaseTrustScoreCommand,
    ) -> Result<Self::Output> {
        if !ctx.ensure_executable(cmd.command_id, cmd.region).await? {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.reward_trust(cmd.amount, cmd.reason)? {
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

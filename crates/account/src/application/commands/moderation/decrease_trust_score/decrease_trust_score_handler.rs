// crates/account/src/application/decrease_trust_score/decrease_trust_score_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::moderation::DecreaseTrustScoreCommand;
use crate::application::context::AccountCommandCtx;

pub struct DecreaseTrustScoreHandler;

impl DecreaseTrustScoreHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for DecreaseTrustScoreHandler {
    type Context = AccountCommandCtx;
    type Command = DecreaseTrustScoreCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: DecreaseTrustScoreCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.penalize_trust(cmd.amount, cmd.reason)? {
            ctx.save(&mut account, cmd.command_id).await?;
        } else {
            info!(
                account_id = %account.account_id(),
                "no changes detected, skipping save"
            );
        }

        Ok(())
    }
}

// crates/account/src/application/increase_trust_score/increase_trust_score_use_case.rs

use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::moderation::increase_trust_score::IncreaseTrustScoreCommand;

pub struct IncreaseTrustScoreHandler;

#[async_trait]
impl CommandHandler for IncreaseTrustScoreHandler {
    type Context = AccountContext;
    type Command = IncreaseTrustScoreCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: IncreaseTrustScoreCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.reward_trust(cmd.amount as i32, cmd.reason)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;
        Ok(())
    }
}
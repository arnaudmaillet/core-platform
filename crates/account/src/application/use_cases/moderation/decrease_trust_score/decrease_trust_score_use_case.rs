// crates/account/src/application/decrease_trust_score/decrease_trust_score_use_case.rs

use async_trait::async_trait;

use shared_kernel::application::CommandHandler;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::moderation::decrease_trust_score::DecreaseTrustScoreCommand;

pub struct DecreaseTrustScoreHandler;

#[async_trait]
impl CommandHandler for DecreaseTrustScoreHandler {
    type Context = AccountContext;
    type Command = DecreaseTrustScoreCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: DecreaseTrustScoreCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.penalize_trust(cmd.amount as i32, cmd.reason)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;
        Ok(())
    }
}

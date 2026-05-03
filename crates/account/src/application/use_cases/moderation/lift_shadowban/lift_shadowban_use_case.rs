// crates/account/src/application/lift_shadowban/lift_shadowban_use_case.rs

use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::moderation::LiftShadowbanCommand;

pub struct LiftShadowbanHandler;

#[async_trait]
impl CommandHandler for LiftShadowbanHandler {
    type Context = AccountContext;
    type Command = LiftShadowbanCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: LiftShadowbanCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.lift_shadowban(cmd.reason)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}
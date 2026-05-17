// crates/account/src/application/lift_shadowban/lift_shadowban_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::moderation::LiftShadowbanCommand;
use crate::application::context::AccountContext;

pub struct LiftShadowbanHandler;

#[async_trait]
impl CommandHandler for LiftShadowbanHandler {
    type Context = AccountContext;
    type Command = LiftShadowbanCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountContext,
        cmd: LiftShadowbanCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.lift_shadowban(cmd.reason)? {
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

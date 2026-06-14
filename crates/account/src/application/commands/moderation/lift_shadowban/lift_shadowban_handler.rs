// crates/account/src/application/lift_shadowban/lift_shadowban_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::moderation::LiftShadowbanCommand;
use crate::application::context::AccountCommandCtx;

pub struct LiftShadowbanHandler;

impl LiftShadowbanHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for LiftShadowbanHandler {
    type Context = AccountCommandCtx;
    type Command = LiftShadowbanCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: LiftShadowbanCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.lift_shadowban(cmd.reason)? {
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

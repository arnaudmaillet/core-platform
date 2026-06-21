// crates/account/src/application/deactivate_account/deactivate_account_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::lifecycle::DeactivateCommand;
use crate::application::context::AccountCommandCtx;

pub struct DeactivateHandler;

impl DeactivateHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for DeactivateHandler {
    type Context = AccountCommandCtx;
    type Command = DeactivateCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: DeactivateCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.deactivate(cmd.reason)? {
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

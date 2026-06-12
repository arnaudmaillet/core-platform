// crates/account/src/application/reactivate_account/reactivate_account_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::lifecycle::ActivateCommand;
use crate::application::context::AccountCommandCtx;

pub struct ActivateHandler;

impl ActivateHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for ActivateHandler {
    type Context = AccountCommandCtx;
    type Command = ActivateCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: ActivateCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;

        if account.activate()? {
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

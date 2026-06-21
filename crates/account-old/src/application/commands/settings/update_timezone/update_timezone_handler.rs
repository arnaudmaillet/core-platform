// crates/account/src/application/update_timezone/mod.rs
use crate::application::commands::settings::UpdateTimezoneCommand;
use crate::application::context::AccountCommandCtx;
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Result, TransactionManager};
use std::marker::PhantomData;
use tracing::info;

pub struct UpdateTimezoneHandler;

impl UpdateTimezoneHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for UpdateTimezoneHandler {
    type Context = AccountCommandCtx;
    type Command = UpdateTimezoneCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: UpdateTimezoneCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.update_timezone(cmd.new_timezone)? {
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

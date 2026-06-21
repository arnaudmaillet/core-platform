// crates/account/src/application/change_role/change_role_use_case.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Result, TransactionManager};
use std::marker::PhantomData;
use tracing::info;

use crate::application::commands::lifecycle::ChangeRoleCommand;
use crate::application::context::AccountCommandCtx;

pub struct ChangeRoleHandler;

impl ChangeRoleHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for ChangeRoleHandler {
    type Context = AccountCommandCtx;
    type Command = ChangeRoleCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: ChangeRoleCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.change_role(cmd.new_role, cmd.reason)? {
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

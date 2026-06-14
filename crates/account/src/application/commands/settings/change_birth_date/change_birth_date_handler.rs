// crates/account/src/application/change_birth_date/change_birth_date_use_case.rs
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use std::marker::PhantomData;
use tracing::info;

use crate::application::commands::settings::ChangeBirthDateCommand;
use crate::application::context::AccountCommandCtx;

pub struct ChangeBirthDateHandler;

impl ChangeBirthDateHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for ChangeBirthDateHandler {
    type Context = AccountCommandCtx;
    type Command = ChangeBirthDateCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: ChangeBirthDateCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.change_birth_date(cmd.new_birth_date)? {
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

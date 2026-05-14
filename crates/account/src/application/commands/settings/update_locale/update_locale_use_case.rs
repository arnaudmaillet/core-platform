// crates/account/src/application/update_locale/update_locale_use_case.rs
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{AggregateRoot, Result};
use tracing::info;

use crate::application::commands::settings::UpdateLocaleCommand;
use crate::application::context::AccountContext;

pub struct UpdateLocaleHandler;

// crates/account/src/application/update_locale/update_locale_use_case.rs

#[async_trait]
impl CommandHandler for UpdateLocaleHandler {
    type Context = AccountContext;
    type Command = UpdateLocaleCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: UpdateLocaleCommand) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;

        if account.update_locale(cmd.new_locale)? {
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

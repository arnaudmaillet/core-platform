// crates/account/src/application/change_birth_date/change_birth_date_use_case.rs
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::commands::settings::ChangeBirthDateCommand;
use crate::application::context::AccountContext;

pub struct ChangeBirthDateHandler;

#[async_trait]
impl CommandHandler for ChangeBirthDateHandler {
    type Context = AccountContext;
    type Command = ChangeBirthDateCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountContext,
        cmd: ChangeBirthDateCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, &cmd.target.region)
            .await?
        {
            return Ok(());
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.change_birth_date(cmd.new_birth_date)? {
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

// crates/account/src/application/add_push_token/add_push_token_use_case.rs
use crate::application::commands::settings::AddPushTokenCommand;
use crate::application::context::AccountCommandCtx;
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

pub struct AddPushTokenHandler;

impl AddPushTokenHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for AddPushTokenHandler {
    type Context = AccountCommandCtx;
    type Command = AddPushTokenCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: AddPushTokenCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.add_push_token(cmd.token)? {
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

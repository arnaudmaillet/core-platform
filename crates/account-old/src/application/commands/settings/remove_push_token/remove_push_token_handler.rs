// crates/account/src/application/remove_push_token/remove_push_token_use_case.rs
use crate::application::commands::settings::RemovePushTokenCommand;
use crate::application::context::AccountCommandCtx;
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

pub struct RemovePushTokenHandler;

impl RemovePushTokenHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for RemovePushTokenHandler {
    type Context = AccountCommandCtx;
    type Command = RemovePushTokenCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: RemovePushTokenCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        if account.remove_push_token(cmd.token)? {
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

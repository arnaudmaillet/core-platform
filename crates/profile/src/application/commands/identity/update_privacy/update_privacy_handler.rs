// crates/profile/src/application/commands/identity/update_privacy/update_privacy_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UpdatePrivacyCommand, context::ProfileContext};

pub struct UpdatePrivacyHandler;

#[async_trait]
impl CommandHandler for UpdatePrivacyHandler {
    type Context = ProfileContext;
    type Command = UpdatePrivacyCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileContext,
        cmd: UpdatePrivacyCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, &cmd.target.region)
            .await?
        {
            return Ok(());
        }

        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_privacy(cmd.is_private)? {
            ctx.save(&mut profile, Some(cmd.command_id)).await?;
        } else {
            info!(
                profile_id = %profile.profile_id(),
                "no changes detected, skipping save"
            );
        }

        Ok(())
    }
}

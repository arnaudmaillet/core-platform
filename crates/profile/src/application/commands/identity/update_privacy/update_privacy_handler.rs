// crates/profile/src/application/commands/identity/update_privacy/update_privacy_handler.rs

use crate::{commands::UpdatePrivacyCommand, context::ProfileCommandContext};
use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

pub struct UpdatePrivacyHandler;

impl UpdatePrivacyHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for UpdatePrivacyHandler {
    type Context = ProfileCommandContext;
    type Command = UpdatePrivacyCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandContext,
        cmd: UpdatePrivacyCommand,
    ) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_privacy(cmd.is_private)? {
            ctx.save(&mut profile).await?;
            info!(
                profile_id = %profile.profile_id(),
                "Privacy updated successfully"
            );
        } else {
            info!(
                profile_id = %profile.profile_id(),
                "Privacy is already the same, skipping save"
            );
        }

        Ok(())
    }
}

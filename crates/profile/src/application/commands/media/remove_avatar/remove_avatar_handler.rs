// crates/profile/src/application/commands/media/remove_avatar/remove_avatar_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::RemoveAvatarCommand, context::ProfileCommandCtx};

pub struct RemoveAvatarHandler;

impl RemoveAvatarHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for RemoveAvatarHandler {
    type Context = ProfileCommandCtx;
    type Command = RemoveAvatarCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandCtx,
        cmd: RemoveAvatarCommand,
    ) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.remove_avatar()? {
            ctx.save(&mut profile, cmd.command_id).await?;

            info!(
                profile_id = %profile.profile_id(),
                "Avatar removed successfully"
            );
        } else {
            info!(
                profile_id = %profile.profile_id(),
                "No avatar detected, skipping save"
            );
        }

        Ok(())
    }
}

// crates/profile/src/application/commands/media/update_avatar/update_avatar_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UpdateAvatarCommand, context::ProfileCommandCtx};

pub struct UpdateAvatarHandler;

impl UpdateAvatarHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for UpdateAvatarHandler {
    type Context = ProfileCommandCtx;
    type Command = UpdateAvatarCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandCtx,
        cmd: UpdateAvatarCommand,
    ) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_avatar(cmd.new_avatar_url)? {
            ctx.save(&mut profile, cmd.command_id).await?;

            info!(
                profile_id = %profile.profile_id(),
                "Avatar updated successfully"
            );
        } else {
            info!(
                profile_id = %profile.profile_id(),
                "Avatar URL is already the same, skipping save"
            );
        }

        Ok(())
    }
}

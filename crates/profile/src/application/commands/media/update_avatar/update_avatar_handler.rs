// crates/profile/src/application/commands/media/update_avatar/update_avatar_handler.rs

use async_trait::async_trait;
use shared_kernel::{application::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UpdateAvatarCommand, context::ProfileContext};

pub struct UpdateAvatarHandler;

#[async_trait]
impl CommandHandler for UpdateAvatarHandler {
    type Context = ProfileContext;
    type Command = UpdateAvatarCommand;
    type Output = ();

    async fn handle(&self, ctx: &ProfileContext, cmd: UpdateAvatarCommand) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_avatar(cmd.new_avatar_url)? {
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

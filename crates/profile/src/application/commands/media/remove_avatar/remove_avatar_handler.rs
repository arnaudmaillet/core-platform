// crates/profile/src/application/commands/media/remove_avatar/remove_avatar_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::RemoveAvatarCommand, context::ProfileContext};

pub struct RemoveAvatarHandler;

#[async_trait]
impl CommandHandler for RemoveAvatarHandler {
    type Context = ProfileContext;
    type Command = RemoveAvatarCommand;
    type Output = ();

    async fn handle(&self, ctx: &ProfileContext, cmd: RemoveAvatarCommand) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.remove_avatar()? {
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

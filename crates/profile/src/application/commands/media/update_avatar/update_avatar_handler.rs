// crates/profile/src/application/commands/media/update_avatar/update_avatar_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UpdateAvatarCommand, context::ProfileCommandContext};

pub struct UpdateAvatarHandler;

#[async_trait]
impl CommandHandler for UpdateAvatarHandler {
    type Context = ProfileCommandContext;
    type Command = UpdateAvatarCommand;
    type Output = ();

    async fn handle(&self, ctx: &ProfileCommandContext, cmd: UpdateAvatarCommand) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }

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

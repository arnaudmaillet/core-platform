// crates/profile/src/application/commands/metadata/update_social_links/update_social_links_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UpdateSocialsCommand, context::ProfileContext};

pub struct UpdateSocialsHandler;

#[async_trait]
impl CommandHandler for UpdateSocialsHandler {
    type Context = ProfileContext;
    type Command = UpdateSocialsCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileContext,
        cmd: UpdateSocialsCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, &cmd.target.region)
            .await?
        {
            return Ok(());
        }

        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_socials(cmd.new_socials)? {
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

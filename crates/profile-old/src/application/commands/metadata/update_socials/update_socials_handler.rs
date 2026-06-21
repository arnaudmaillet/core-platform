// crates/profile/src/application/commands/metadata/update_social_links/update_social_links_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UpdateSocialsCommand, context::ProfileCommandCtx};

pub struct UpdateSocialsHandler;

impl UpdateSocialsHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for UpdateSocialsHandler {
    type Context = ProfileCommandCtx; // Contexte épuré Full ScyllaDB
    type Command = UpdateSocialsCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandCtx,
        cmd: UpdateSocialsCommand,
    ) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;
        if profile.update_socials(cmd.new_socials)? {
            ctx.save(&mut profile, cmd.command_id).await?;

            info!(
                profile_id = %profile.profile_id(),
                "Social links updated successfully"
            );
        } else {
            info!(
                profile_id = %profile.profile_id(),
                "Social links are already identical, skipping save"
            );
        }

        Ok(())
    }
}

// crates/profile/src/application/commands/metadata/update_social_links/update_social_links_handler.rs

use async_trait::async_trait;
use shared_kernel::{application::CommandHandler, errors::Result};
use tracing::info;

use crate::{commands::UpdateSocialLinksCommand, context::ProfileContext};

pub struct UpdateSocialLinksHandler;

#[async_trait]
impl CommandHandler for UpdateSocialLinksHandler {
    type Context = ProfileContext;
    type Command = UpdateSocialLinksCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileContext,
        cmd: UpdateSocialLinksCommand,
    ) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_social_links(cmd.new_links)? {
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

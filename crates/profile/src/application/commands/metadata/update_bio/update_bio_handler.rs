// crates/profile/src/application/commands/metadata/update_bio/update_bio_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UpdateBioCommand, context::ProfileContext};
pub struct UpdateBioHandler;

#[async_trait]
impl CommandHandler for UpdateBioHandler {
    type Context = ProfileContext;
    type Command = UpdateBioCommand;
    type Output = ();

    async fn handle(&self, ctx: &ProfileContext, cmd: UpdateBioCommand) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_bio(cmd.new_bio)? {
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

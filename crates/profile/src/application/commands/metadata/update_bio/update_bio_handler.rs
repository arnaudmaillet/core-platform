// crates/profile/src/application/commands/metadata/update_bio/update_bio_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UpdateBioCommand, context::ProfileCommandContext};

pub struct UpdateBioHandler;

impl UpdateBioHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for UpdateBioHandler {
    type Context = ProfileCommandContext;
    type Command = UpdateBioCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandContext,
        cmd: UpdateBioCommand,
    ) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;
        if profile.update_bio(cmd.new_bio)? {
            ctx.save(&mut profile).await?;

            info!(
                profile_id = %profile.profile_id(),
                "Biography updated successfully"
            );
        } else {
            info!(
                profile_id = %profile.profile_id(),
                "Biography is already identical, skipping save"
            );
        }

        Ok(())
    }
}

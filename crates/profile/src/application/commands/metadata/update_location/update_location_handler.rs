// crates/profile/src/application/commands/metadata/update_location_label/update_location_label_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UpdateLocationCommand, context::ProfileCommandCtx};

pub struct UpdateLocationHandler;

impl UpdateLocationHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for UpdateLocationHandler {
    type Context = ProfileCommandCtx;
    type Command = UpdateLocationCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandCtx,
        cmd: UpdateLocationCommand,
    ) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_location(cmd.new_location)? {
            ctx.save(&mut profile, cmd.command_id).await?;

            info!(
                profile_id = %profile.profile_id(),
                "Location updated successfully"
            );
        } else {
            info!(
                profile_id = %profile.profile_id(),
                "Location is already identical, skipping save"
            );
        }

        Ok(())
    }
}

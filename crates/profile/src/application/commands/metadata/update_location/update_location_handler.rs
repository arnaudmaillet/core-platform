// crates/profile/src/application/commands/metadata/update_location_label/update_location_label_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UpdateLocationCommand, context::ProfileContext};

pub struct UpdateLocationHandler;

#[async_trait]
impl CommandHandler for UpdateLocationHandler {
    type Context = ProfileContext;
    type Command = UpdateLocationCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileContext,
        cmd: UpdateLocationCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, &cmd.target.region)
            .await?
        {
            return Ok(());
        }

        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_location(cmd.new_location)? {
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

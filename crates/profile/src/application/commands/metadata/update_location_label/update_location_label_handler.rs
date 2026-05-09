// crates/profile/src/application/commands/metadata/update_location_label/update_location_label_handler.rs

use async_trait::async_trait;
use shared_kernel::{application::CommandHandler, errors::Result};
use tracing::info;

use crate::{commands::UpdateLocationLabelCommand, context::ProfileContext};

pub struct UpdateLocationLabelHandler;

#[async_trait]
impl CommandHandler for UpdateLocationLabelHandler {
    type Context = ProfileContext;
    type Command = UpdateLocationLabelCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileContext,
        cmd: UpdateLocationLabelCommand,
    ) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_location_label(cmd.new_location_label)? {
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

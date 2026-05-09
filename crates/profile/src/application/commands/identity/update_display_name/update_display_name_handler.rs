// crates/profile/src/application/commands/identity/update_handle/update_handle_handler.rs

use async_trait::async_trait;
use shared_kernel::{application::CommandHandler, errors::Result};
use tracing::info;

use crate::{commands::UpdateDisplayNameCommand, context::ProfileContext};

pub struct UpdateDisplayNameHandler;

#[async_trait]
impl CommandHandler for UpdateDisplayNameHandler {
    type Context = ProfileContext;
    type Command = UpdateDisplayNameCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileContext,
        cmd: UpdateDisplayNameCommand,
    ) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_display_name(cmd.new_display_name)? {
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

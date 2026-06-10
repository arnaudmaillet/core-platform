// crates/profile/src/application/commands/identity/update_handle/update_handle_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UpdateDisplayNameCommand, context::ProfileCommandContext};

pub struct UpdateDisplayNameHandler;

impl UpdateDisplayNameHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for UpdateDisplayNameHandler {
    type Context = ProfileCommandContext;
    type Command = UpdateDisplayNameCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandContext,
        cmd: UpdateDisplayNameCommand,
    ) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_display_name(cmd.new_display_name)? {
            ctx.save(&mut profile).await?;

            info!(
                profile_id = %profile.profile_id(),
                "Display name updated successfully"
            );
        } else {
            info!(
                profile_id = %profile.profile_id(),
                "Display name is already the same, skipping save"
            );
        }

        Ok(())
    }
}

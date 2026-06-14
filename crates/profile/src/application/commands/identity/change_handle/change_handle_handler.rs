// crates/profile/src/application/commands/identity/change_handle/change_handle_handler.rs

use crate::{commands::ChangeHandleCommand, context::ProfileCommandCtx};
use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};

pub struct ChangeHandleHandler;

impl ChangeHandleHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for ChangeHandleHandler {
    type Context = ProfileCommandCtx;
    type Command = ChangeHandleCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandCtx,
        cmd: ChangeHandleCommand,
    ) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.handle() == &cmd.new_handle {
            tracing::info!(
                profile_id = %profile.profile_id(),
                "Handle is already the same, skipping validation and save"
            );
            return Ok(());
        }

        let old_slug_hash = profile.handle().to_sha256_hash();
        let new_slug_hash = cmd.new_handle.to_sha256_hash();

        profile.change_handle(cmd.new_handle)?;

        ctx.routing_repo()
            .update_slug_routing(
                profile.profile_id(),
                &old_slug_hash,
                &new_slug_hash,
                ctx.server_region(),
            )
            .await?;

        ctx.save(&mut profile, cmd.command_id).await?;

        Ok(())
    }
}

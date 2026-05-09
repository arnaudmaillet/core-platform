// crates/profile/src/application/commands/identity/update_handle/update_handle_handler.rs

use async_trait::async_trait;
use shared_kernel::{
    application::CommandHandler,
    errors::{DomainError, Result},
};

use crate::{commands::UpdateHandleCommand, context::ProfileContext};

pub struct UpdateHandleHandler;

#[async_trait]
impl CommandHandler for UpdateHandleHandler {
    type Context = ProfileContext;
    type Command = UpdateHandleCommand;
    type Output = ();

    async fn handle(&self, ctx: &ProfileContext, cmd: UpdateHandleCommand) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.handle() == &cmd.new_handle {
            tracing::info!(
                profile_id = %profile.profile_id(),
                "handle is already the same, skipping validation and save"
            );
            return Ok(());
        }

        if ctx
            .profile_repo()
            .find_by_handle(&cmd.new_handle, ctx.region(), None)
            .await?
            .is_some()
        {
            return Err(DomainError::AlreadyExists {
                entity: "Profile",
                field: "handle".into(),
                value: cmd.new_handle.as_str().to_string(),
            });
        }

        profile.update_handle(cmd.new_handle)?;

        ctx.save(&mut profile, Some(cmd.command_id)).await?;

        Ok(())
    }
}

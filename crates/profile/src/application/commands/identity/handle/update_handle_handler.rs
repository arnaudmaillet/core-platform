// crates/profile/src/application/commands/identity/handle/update_handle_handler.rs

use async_trait::async_trait;
use shared_kernel::{
    application::CommandHandler,
    errors::{DomainError, Result},
};

use crate::application::{
    context::context::ProfileContext, use_cases::update_handle::UpdateHandleCommand,
};

pub struct UpdateHandleHandler;

#[async_trait]
impl CommandHandler for UpdateHandleHandler {
    type Context = ProfileContext;
    type Command = UpdateHandleCommand;
    type Output = ();

    async fn handle(&self, ctx: &ProfileContext, cmd: UpdateHandleCommand) -> Result<Self::Output> {
        let mut profile = ctx.profile().await?;

        if profile.version() != cmd.expected_version {
            return Err(DomainError::ConcurrencyConflict {
                reason: format!(
                    "OCC Mismatch: expected v{}, got v{}",
                    cmd.expected_version,
                    profile.version()
                ),
            });
        }

        if ctx
            .repo()
            .find_by_handle(&cmd.new_handle, &cmd.region, None)
            .await?
            .is_some()
        {
            return Err(DomainError::AlreadyExists(format!(
                "Handle '{}' is already taken",
                cmd.new_handle.as_str()
            )));
        }

        profile.update_handle(cmd.new_handle)?;

        ctx.save(&mut profile, Some(cmd.command_id)).await?;

        Ok(())
    }
}

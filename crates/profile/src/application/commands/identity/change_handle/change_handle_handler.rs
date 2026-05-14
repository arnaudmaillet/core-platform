// crates/profile/src/application/commands/identity/change_handle/change_handle_handler.rs

use async_trait::async_trait;
use shared_kernel::{
    command::CommandHandler,
    core::{Error, Result},
};

use crate::{commands::ChangeHandleCommand, context::ProfileContext};

pub struct ChangeHandleHandler;

#[async_trait]
impl CommandHandler for ChangeHandleHandler {
    type Context = ProfileContext;
    type Command = ChangeHandleCommand;
    type Output = ();

    async fn handle(&self, ctx: &ProfileContext, cmd: ChangeHandleCommand) -> Result<Self::Output> {
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
            return Err(Error::already_exists(
                "Profile",
                "handle".into(),
                cmd.new_handle.as_str().to_string(),
            ));
        }
        profile.change_handle(cmd.new_handle)?;

        ctx.save(&mut profile, Some(cmd.command_id)).await?;

        Ok(())
    }
}

// crates/profile/src/application/commands/identity/change_handle/change_handle_handler.rs

use crate::{commands::ChangeHandleCommand, context::ProfileCommandContext};
use async_trait::async_trait;
use shared_kernel::{
    command::CommandHandler,
    core::{Error, Result, TransactionManager},
};
use std::marker::PhantomData;

pub struct ChangeHandleHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> ChangeHandleHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for ChangeHandleHandler<TM> {
    type Context = ProfileCommandContext<TM>;
    type Command = ChangeHandleCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandContext<TM>,
        cmd: ChangeHandleCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.region)
            .await?
        {
            return Ok(());
        }

        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.handle() == &cmd.new_handle {
            tracing::info!(
                profile_id = %profile.profile_id(),
                "handle is already the same, skipping validation and save"
            );
            return Ok(());
        }

        if ctx.exists_by_handle(&cmd.new_handle).await? {
            return Err(Error::already_exists(
                "Profile",
                "handle",
                cmd.new_handle.as_str().to_string(),
            ));
        }

        profile.change_handle(cmd.new_handle)?;

        ctx.save(&mut profile, Some(cmd.command_id)).await?;

        Ok(())
    }
}

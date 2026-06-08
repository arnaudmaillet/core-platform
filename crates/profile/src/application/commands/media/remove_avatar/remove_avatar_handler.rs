// crates/profile/src/application/commands/media/remove_avatar/remove_avatar_handler.rs

use std::marker::PhantomData;

use crate::{commands::RemoveAvatarCommand, context::ProfileCommandContext};
use async_trait::async_trait;
use shared_kernel::{
    command::CommandHandler,
    core::{Result, TransactionManager},
};
use tracing::info;

pub struct RemoveAvatarHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> RemoveAvatarHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for RemoveAvatarHandler<TM> {
    type Context = ProfileCommandContext<TM>;
    type Command = RemoveAvatarCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandContext<TM>,
        cmd: RemoveAvatarCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.region)
            .await?
        {
            return Ok(());
        }

        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.remove_avatar()? {
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

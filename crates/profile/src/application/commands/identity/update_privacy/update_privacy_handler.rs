// crates/profile/src/application/commands/identity/update_privacy/update_privacy_handler.rs

use std::marker::PhantomData;

use async_trait::async_trait;
use shared_kernel::{
    command::CommandHandler,
    core::{Result, TransactionManager},
};
use tracing::info;

use crate::{commands::UpdatePrivacyCommand, context::ProfileCommandContext};

pub struct UpdatePrivacyHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> UpdatePrivacyHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for UpdatePrivacyHandler<TM> {
    type Context = ProfileCommandContext<TM>;
    type Command = UpdatePrivacyCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandContext<TM>,
        cmd: UpdatePrivacyCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.region)
            .await?
        {
            return Ok(());
        }

        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_privacy(cmd.is_private)? {
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

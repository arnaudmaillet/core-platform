// crates/profile/src/application/commands/metadata/update_location_label/update_location_label_handler.rs

use std::marker::PhantomData;

use async_trait::async_trait;
use shared_kernel::{
    command::CommandHandler,
    core::{Result, TransactionManager},
};
use tracing::info;

use crate::{commands::UpdateLocationCommand, context::ProfileCommandContext};

pub struct UpdateLocationHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> UpdateLocationHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for UpdateLocationHandler<TM> {
    type Context = ProfileCommandContext<TM>;
    type Command = UpdateLocationCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandContext<TM>,
        cmd: UpdateLocationCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.region)
            .await?
        {
            return Ok(());
        }

        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_location(cmd.new_location)? {
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

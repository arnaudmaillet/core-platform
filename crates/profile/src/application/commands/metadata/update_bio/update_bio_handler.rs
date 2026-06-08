// crates/profile/src/application/commands/metadata/update_bio/update_bio_handler.rs

use std::marker::PhantomData;

use crate::{commands::UpdateBioCommand, context::ProfileCommandContext};
use async_trait::async_trait;
use shared_kernel::{
    command::CommandHandler,
    core::{Result, TransactionManager},
};
use tracing::info;

pub struct UpdateBioHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> UpdateBioHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for UpdateBioHandler<TM> {
    type Context = ProfileCommandContext<TM>;
    type Command = UpdateBioCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandContext<TM>,
        cmd: UpdateBioCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.region)
            .await?
        {
            return Ok(());
        }

        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_bio(cmd.new_bio)? {
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

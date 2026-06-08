// crates/profile/src/application/commands/metadata/update_social_links/update_social_links_handler.rs

use std::marker::PhantomData;

use async_trait::async_trait;
use shared_kernel::{
    command::CommandHandler,
    core::{Result, TransactionManager},
};
use tracing::info;

use crate::{commands::UpdateSocialsCommand, context::ProfileCommandContext};

pub struct UpdateSocialsHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> UpdateSocialsHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for UpdateSocialsHandler<TM> {
    type Context = ProfileCommandContext<TM>;
    type Command = UpdateSocialsCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandContext<TM>,
        cmd: UpdateSocialsCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.region)
            .await?
        {
            return Ok(());
        }

        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_socials(cmd.new_socials)? {
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

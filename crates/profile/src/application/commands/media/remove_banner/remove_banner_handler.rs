// crates/profile/src/application/commands/media/remove_banner/remove_banner_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::RemoveBannerCommand, context::ProfileCommandContext};

pub struct RemoveBannerHandler;

impl RemoveBannerHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for RemoveBannerHandler {
    type Context = ProfileCommandContext;
    type Command = RemoveBannerCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandContext,
        cmd: RemoveBannerCommand,
    ) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.remove_banner()? {
            ctx.save(&mut profile).await?;

            info!(
                profile_id = %profile.profile_id(),
                "Banner removed successfully"
            );
        } else {
            info!(
                profile_id = %profile.profile_id(),
                "No banner detected, skipping save"
            );
        }

        Ok(())
    }
}

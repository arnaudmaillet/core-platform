// crates/post/src/application/handlers/update_caption_handler.rs

use crate::post::{
    application::{context::PostCommandCtx, handlers::UpdateCaptionCommand},
    domain::types::Mentions,
};
use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

pub struct UpdateCaptionHandler;

#[async_trait]
impl CommandHandler for UpdateCaptionHandler {
    type Context = PostCommandCtx;
    type Command = UpdateCaptionCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &PostCommandCtx,
        cmd: UpdateCaptionCommand,
    ) -> Result<Self::Output> {
        // let slugs = cmd
        //     .new_caption
        //     .as_ref()
        //     .map(|c| c.extract_mentions())
        //     .unwrap_or_default();
        // let profile_map = ctx
        //     .kernel()
        //     .profile_resolver()
        //     .resolve_slugs(&slugs)
        //     .await?;
        // let resolved_profiles = profile_map.values().cloned().collect();
        // let mentions = Mentions::try_new(resolved_profiles)?;

        // let mut post = ctx.fetch_verified(&cmd.target).await?;
        // if post.update_caption(cmd.new_caption, mentions)? {
        //     ctx.save(&mut post, cmd.command_id).await?;
        // } else {
        //     info!(
        //         post_id = %post.post_id(),
        //         "no changes detected, skipping save"
        //     );
        // }
        Ok(())
    }
}

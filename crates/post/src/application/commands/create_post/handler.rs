// crates/post/src/application/handlers/create_post_handler.rs

use std::collections::BTreeSet;

use crate::{
    application::{commands::CreatePostCommand, context::PostCommandContext},
    domain::entities::Post,
    types::{DynamicMetadata, Hashtags, Mentions},
};
use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
pub struct CreatePostHandler;

#[async_trait]
impl CommandHandler for CreatePostHandler {
    type Context = PostCommandContext;
    type Command = CreatePostCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &PostCommandContext,
        cmd: CreatePostCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.region)
            .await?
        {
            return Ok(());
        }

        let mut extracted_slugs = BTreeSet::new();
        let mut extracted_tags = BTreeSet::new();

        if let Some(cap) = &cmd.caption {
            extracted_slugs = cap.extract_mentions();
            extracted_tags = cap.extract_hashtags();
        }

        let mut resolved_profiles = BTreeSet::new();
        if !extracted_slugs.is_empty() {
            let profile_map = ctx
                .app()
                .profile_resolver()
                .resolve_slugs(&extracted_slugs)
                .await?;
            for (_slug, profile_id) in profile_map {
                resolved_profiles.insert(profile_id);
            }
        }

        let mentions = Mentions::try_new(resolved_profiles)?;
        let hashtags = Hashtags::try_from(extracted_tags.into_iter().collect::<Vec<String>>())?;

        let mut post = Post::builder(
            cmd.post_id,
            cmd.target.id,
            cmd.post_type,
            cmd.visibility_level.parse()?,
        )
        .with_media_list(cmd.media_list)
        .with_optional_caption(cmd.caption)
        .with_comment_settings(cmd.allowed_comment_hands)
        .with_dynamic_metadata(cmd.dynamic_metadata.unwrap_or_else(DynamicMetadata::empty))
        .with_optional_music_id(cmd.music_id)
        .with_mentions(mentions)
        .with_hashtags(hashtags)
        .build()?;

        ctx.save(&mut post, Some(cmd.command_id)).await?;

        Ok(())
    }
}

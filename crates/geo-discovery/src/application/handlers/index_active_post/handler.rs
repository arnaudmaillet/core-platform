// crates/geo_discovery/src/application/handlers/index_active_post_handler.rs

use async_trait::async_trait;
use std::str::FromStr;

use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use shared_kernel::types::PostType;

use crate::context::GeoDiscoveryCommandContext;
use crate::domain::types::TilePostMetadata;
use crate::handlers::IndexActivePostCommand;

pub struct IndexActivePostHandler;

#[async_trait]
impl CommandHandler for IndexActivePostHandler {
    type Context = GeoDiscoveryCommandContext;
    type Command = IndexActivePostCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &GeoDiscoveryCommandContext,
        cmd: IndexActivePostCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }

        let post_type = PostType::from_str(&cmd.post_type)?;
        let thumbnail_url = cmd.thumbnail_url.filter(|url| !url.is_empty());

        let metadata = TilePostMetadata::new(
            cmd.post_id,
            cmd.location.lat(),
            cmd.location.lon(),
            post_type,
            thumbnail_url,
        );

        ctx.index_active_post(
            metadata,
            cmd.location,
            cmd.created_at,
            cmd.expires_at,
            cmd.initial_score,
            Some(cmd.command_id),
        )
        .await?;

        Ok(())
    }
}

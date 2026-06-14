// crates/geo_discovery/src/application/handlers/index_active_post_handler.rs

use async_trait::async_trait;
use std::str::FromStr;

use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use shared_kernel::types::PostType;

use crate::use_cases::IndexMapAnnotationCommand;
use crate::context::GeoDiscoveryCommandCtx;
use crate::domain::types::TilePostMetadata;
pub struct IndexMapAnnotationHandler;

#[async_trait]
impl CommandHandler for IndexMapAnnotationHandler {
    type Context = GeoDiscoveryCommandCtx;
    type Command = IndexMapAnnotationCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &GeoDiscoveryCommandCtx,
        cmd: IndexMapAnnotationCommand,
    ) -> Result<Self::Output> {
        ctx.verify_region(cmd.region)?;

        let post_type: PostType = PostType::from_str(&cmd.post_type)?;
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
            cmd.popularity_score,
            Some(cmd.command_id),
        )
        .await?;

        Ok(())
    }
}

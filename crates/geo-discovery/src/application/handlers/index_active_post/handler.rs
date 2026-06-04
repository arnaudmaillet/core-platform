// crates/geo_discovery/src/application/handlers/index_active_post_handler.rs

use async_trait::async_trait;
use std::time::Duration;

use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;

use crate::context::GeoDiscoveryCommandContext;
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

        let ttl_duration = Duration::from_hours(48);

        ctx.index_active_post(
            cmd.post_id,
            cmd.location,
            cmd.created_at,
            cmd.initial_score,
            ttl_duration,
            Some(cmd.command_id),
        )
        .await?;

        Ok(())
    }
}

// crates/geo_discovery/src/application/handlers/remove_post_from_map_handler.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;

use crate::context::GeoDiscoveryCommandContext;
use crate::handlers::RemovePostFromMapCommand;

pub struct RemovePostFromMapHandler;

#[async_trait]
impl CommandHandler for RemovePostFromMapHandler {
    type Context = GeoDiscoveryCommandContext;
    type Command = RemovePostFromMapCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &GeoDiscoveryCommandContext,
        cmd: RemovePostFromMapCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }

        ctx.remove_post_from_map(cmd.location, cmd.created_at, &cmd.post_id, cmd.command_id)
            .await?;

        Ok(())
    }
}

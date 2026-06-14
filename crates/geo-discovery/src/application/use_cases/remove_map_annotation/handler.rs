// crates/geo_discovery/src/application/handlers/remove_post_from_map_handler.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;

use crate::context::GeoDiscoveryCommandCtx;
use crate::use_cases::RemoveMapAnnotationCommand;

pub struct RemoveMapAnnotationHandler;

#[async_trait]
impl CommandHandler for RemoveMapAnnotationHandler {
    type Context = GeoDiscoveryCommandCtx;
    type Command = RemoveMapAnnotationCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &GeoDiscoveryCommandCtx,
        cmd: RemoveMapAnnotationCommand,
    ) -> Result<Self::Output> {
        ctx.verify_region(cmd.region)?;
        ctx.remove_post_from_map(cmd.location, cmd.created_at, &cmd.post_id, cmd.command_id)
            .await?;

        Ok(())
    }
}

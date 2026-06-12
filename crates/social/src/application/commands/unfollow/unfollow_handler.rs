use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UnfollowCommand, context::SocialCommandContext, entities::FollowRelation};

pub struct UnfollowHandler;

#[async_trait]
impl CommandHandler for UnfollowHandler {
    type Context = SocialCommandContext;
    type Command = UnfollowCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &SocialCommandContext,
        cmd: UnfollowCommand,
    ) -> Result<Self::Output> {
        ctx.verify_actors(cmd.follower_id, cmd.target.id)?;

        if cmd.follower_id == cmd.target.id {
            return Ok(());
        }

        let query_ctx = ctx.app().query(ctx.region());
        let is_following = query_ctx
            .is_already_following(cmd.follower_id, cmd.target.id)
            .await?;

        if !is_following {
            info!(
                follower_id = %cmd.follower_id,
                following_id = %cmd.target.id,
                "User is not following this profile, skipping execute"
            );
            return Ok(());
        }

        let mut relation = FollowRelation::builder(cmd.follower_id, cmd.target.id).build()?;

        if relation.execute_unfollow()? {
            ctx.delete_relation(&mut relation).await?;
        }

        Ok(())
    }
}

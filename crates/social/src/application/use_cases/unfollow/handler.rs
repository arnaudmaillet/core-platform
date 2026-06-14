use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{context::SocialCommandCtx, entities::FollowRelation, use_cases::UnfollowCommand};

pub struct UnfollowHandler;

#[async_trait]
impl CommandHandler for UnfollowHandler {
    type Context = SocialCommandCtx;
    type Command = UnfollowCommand;
    type Output = ();

    async fn handle(&self, ctx: &SocialCommandCtx, cmd: UnfollowCommand) -> Result<Self::Output> {
        ctx.verify_actors(cmd.follower_id, cmd.target.id)?;

        if cmd.follower_id == cmd.target.id {
            return Ok(());
        }

        if !ctx
            .is_already_following(cmd.follower_id, cmd.target.id)
            .await?
        {
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

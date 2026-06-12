// crates/social/src/application/commands/follow/follow_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{
    commands::FollowCommand, context::SocialCommandContext, domain::entities::FollowRelation,
};

pub struct FollowHandler;

#[async_trait]
impl CommandHandler for FollowHandler {
    type Context = SocialCommandContext;
    type Command = FollowCommand;
    type Output = ();

    async fn handle(&self, ctx: &SocialCommandContext, cmd: FollowCommand) -> Result<Self::Output> {
        ctx.verify_actors(cmd.follower_id, cmd.target.id)?;

        if cmd.follower_id == cmd.target.id {
            return Ok(());
        }
        let query_ctx = ctx.app().query(cmd.region);
        let already_following = query_ctx
            .is_already_following(cmd.follower_id, cmd.target.id)
            .await?;

        if already_following {
            info!(
                follower_id = %cmd.follower_id,
                following_id = %cmd.target.id,
                "user is already following this profile, skipping execute"
            );
            return Ok(());
        }

        let mut relation = FollowRelation::builder(cmd.follower_id, cmd.target.id).build()?;

        if relation.execute_follow()? {
            ctx.save_relation(&mut relation).await?;
        }

        Ok(())
    }
}

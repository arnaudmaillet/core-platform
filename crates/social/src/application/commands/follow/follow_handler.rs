use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::FollowCommand, context::SocialContext, domain::entities::FollowRelation};

pub struct FollowHandler;

#[async_trait]
impl CommandHandler for FollowHandler {
    type Context = SocialContext;
    type Command = FollowCommand;
    type Output = ();

    async fn handle(&self, ctx: &SocialContext, cmd: FollowCommand) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, &cmd.target.region)
            .await?
        {
            return Ok(());
        }

        if cmd.follower_id == cmd.target.id {
            return Ok(());
        }

        let already_following = ctx
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
            ctx.save_relation(&mut relation, cmd.command_id).await?;
        }

        Ok(())
    }
}

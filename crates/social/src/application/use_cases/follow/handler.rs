// crates/social/src/application/commands/follow/follow_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{
    context::SocialCommandCtx, domain::entities::FollowRelation, use_cases::FollowCommand,
};

pub struct FollowHandler;

#[async_trait]
impl CommandHandler for FollowHandler {
    type Context = SocialCommandCtx;
    type Command = FollowCommand;
    type Output = ();

    async fn handle(&self, ctx: &SocialCommandCtx, cmd: FollowCommand) -> Result<Self::Output> {
        ctx.verify_actors(cmd.follower_id, cmd.target.id)?;

        if cmd.follower_id == cmd.target.id {
            return Ok(());
        }

        if ctx
            .is_already_following(cmd.follower_id, cmd.target.id)
            .await?
        {
            info!(
                follower_id = %cmd.follower_id,
                following_id = %cmd.target.id,
                "L'utilisateur suit déjà ce profil, abandon de l'exécution"
            );
            return Ok(());
        }

        // 3. Exécution métier de l'agrégat / de l'entité de domaine
        let mut relation = FollowRelation::builder(cmd.follower_id, cmd.target.id).build()?;

        if relation.execute_follow()? {
            ctx.save_relation(&mut relation).await?;
        }

        Ok(())
    }
}

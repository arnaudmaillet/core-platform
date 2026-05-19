use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UnfollowCommand, context::SocialContext, entities::FollowRelation};

pub struct UnfollowHandler;

#[async_trait]
impl CommandHandler for UnfollowHandler {
    type Context = SocialContext;
    type Command = UnfollowCommand;
    type Output = ();

    async fn handle(&self, ctx: &SocialContext, cmd: UnfollowCommand) -> Result<Self::Output> {
        // 1. Contrôle de l'idempotence et du partitionnement régional
        if !ctx
            .ensure_executable(cmd.command_id, &cmd.target.region)
            .await?
        {
            return Ok(());
        }

        // 2. Invariant métier de base : impossible de s'unfollow soi-même
        if cmd.follower_id == cmd.target.id {
            return Ok(());
        }

        // 3. Vérification de l'existence de la relation
        let is_following = ctx
            .is_already_following(cmd.follower_id, cmd.target.id)
            .await?;
        if !is_following {
            info!(
                follower_id = %cmd.follower_id,
                following_id = %cmd.target.id,
                "user is not following this profile, skipping execute"
            );
            return Ok(());
        }

        // 4. Reconstitution de l'agrégat via son Builder à sa version par défaut
        let mut relation = FollowRelation::builder(cmd.follower_id, cmd.target.id).build()?;

        if relation.execute_unfollow()? {
            ctx.delete_relation(&mut relation, cmd.command_id).await?;
        }

        Ok(())
    }
}

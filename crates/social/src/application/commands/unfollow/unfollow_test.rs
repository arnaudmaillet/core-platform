#[cfg(test)]
mod tests {
    use crate::commands::UnfollowCommand;
    use crate::context::SocialContext;
    use crate::repositories::CounterRepository;
    use crate::test_utils::SocialTestFixture;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::Result;
    use shared_kernel::idempotency::IdempotencyRepository;
    use shared_kernel::types::ProfileId;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_unfollow_handler_success_nominal_path() -> Result<()> {
        // Arrange
        let f = SocialTestFixture::new();
        let follower_id = ProfileId::generate(f.region());
        let target_id = f.target_profile_id();

        // ÉTAT INITIAL : La relation existe et les compteurs ont déjà été incrémentés
        f.given_existing_relation(follower_id, target_id);
        f.given_initial_counters(follower_id, 0, 1);
        f.given_initial_counters(target_id, 1, 0);

        let cmd = UnfollowCommand {
            command_id: Uuid::new_v4(),
            follower_id,
            target: CommandTarget::new(target_id, f.region(), 1),
        };

        // Act
        f.bus()
            .execute::<SocialContext, UnfollowCommand, ()>(f.social_ctx().clone(), cmd)
            .await?;

        // Assert
        // 1. La relation a bien été purgée de ScyllaDB
        f.assert_relation_does_not_exist(follower_id, target_id)
            .await;

        // 2. Le Hot Path Redis a décrémenté les compteurs de manière atomique
        f.assert_counters_values(follower_id, 0, 0).await;
        f.assert_counters_values(target_id, 0, 0).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_unfollow_handler_should_abort_silently_when_idempotency_barrier_triggers()
    -> Result<()> {
        // Arrange
        let f = SocialTestFixture::new();
        let command_id = Uuid::new_v4();
        let follower_id = ProfileId::generate(f.region());
        let target_id = f.target_profile_id();

        // On injecte la relation initiale
        f.given_existing_relation(follower_id, target_id);

        // On simule une commande déjà traitée dans le verrou d'idempotence technique
        f.idempotency_repo().save(None, &command_id).await?;

        let cmd = UnfollowCommand {
            command_id,
            follower_id,
            target: CommandTarget::new(target_id, f.region(), 1),
        };

        // Act
        f.bus()
            .execute::<SocialContext, UnfollowCommand, ()>(f.social_ctx().clone(), cmd)
            .await?;

        // Assert
        // L'idempotence a bloqué : la relation doit TOUJOURS exister
        f.assert_relation_exists(follower_id, target_id).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_unfollow_handler_should_ignore_self_unfollowing_attempts() -> Result<()> {
        // Arrange
        let f = SocialTestFixture::new();
        let actor_id = f.target_profile_id();

        // Invariant : follower_id == target_id
        let cmd = UnfollowCommand {
            command_id: Uuid::new_v4(),
            follower_id: actor_id,
            target: CommandTarget::new(actor_id, f.region(), 1),
        };

        // Act
        f.bus()
            .execute::<SocialContext, UnfollowCommand, ()>(f.social_ctx().clone(), cmd)
            .await?;

        // Assert
        // Rien ne bouge
        f.assert_relation_does_not_exist(actor_id, actor_id).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_unfollow_handler_should_skip_execution_if_not_following() -> Result<()> {
        // Arrange
        let f = SocialTestFixture::new();
        let command_id = Uuid::new_v4();
        let follower_id = ProfileId::generate(f.region());
        let target_id = f.target_profile_id();

        // ÉTAT INITIAL : Pas de relation en base, compteurs vierges à 0
        f.given_initial_counters(follower_id, 0, 0);
        f.given_initial_counters(target_id, 0, 0);

        let cmd = UnfollowCommand {
            command_id,
            follower_id,
            target: CommandTarget::new(target_id, f.region(), 1),
        };

        // Act
        f.bus()
            .execute::<SocialContext, UnfollowCommand, ()>(f.social_ctx().clone(), cmd)
            .await?;

        // Assert
        // L'idempotence technique n'a pas été posée (skip précoce car relation inexistante)
        assert!(!f.idempotency_repo().exists(None, &command_id).await?);

        // Les compteurs restent bloqués à 0 (grâce au saturating_sub de ton Counter et au skip du handler)
        let redis_follower = f.cache_counter_repo().get_counters(follower_id).await?;
        assert_eq!(redis_follower.following_count().value(), 0);

        Ok(())
    }
}

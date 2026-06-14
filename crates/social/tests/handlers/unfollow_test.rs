use shared_kernel::command::CommandTarget;
use shared_kernel::core::Result;
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::ProfileId;
use social::context::SocialCommandCtx;
use social::events::SocialEvent;
use social::use_cases::UnfollowCommand;
use social_test_utils::SocialTestFixture;
use social_test_utils::assertions::{CounterRepositoryAsserts, RelationRepositoryAsserts};
use uuid::Uuid;

#[tokio::test]
async fn test_unfollow_handler_success_nominal_path() -> Result<()> {
    // Arrange
    let f = SocialTestFixture::new();
    let follower_id = ProfileId::generate();
    let target_id = f.target_profile_id();

    f.given_existing_relation(follower_id, target_id);
    f.given_initial_counters(follower_id, 0, 1);
    f.given_initial_counters(target_id, 1, 0);

    // 🚀 AJUSTEMENT RUST : Le champ region est retiré de la commande
    let cmd = UnfollowCommand {
        command_id: Uuid::new_v4(),
        follower_id,
        target: CommandTarget::stateless(target_id),
        region: f.region(),
    };

    // Act
    f.bus()
        .execute::<SocialCommandCtx, UnfollowCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.relation_repo()
        .assert_relation_does_not_exist(follower_id, target_id)
        .await;

    f.cache_counter_repo()
        .assert_counters_values(follower_id, 0, 0)
        .await;
    f.cache_counter_repo()
        .assert_counters_values(target_id, 0, 0)
        .await;

    f.relation_repo()
        .assert_captured_event_for(follower_id, |event| match event {
            SocialEvent::ProfileUnfollowed {
                follower_id: f_id,
                following_id: tg_id,
                ..
            } => {
                assert_eq!(*f_id, follower_id);
                assert_eq!(*tg_id, target_id);
            }
            _ => panic!("Type d'événement incorrect capturé : attendu ProfileUnfollowed"),
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_unfollow_handler_should_abort_silently_when_idempotency_barrier_triggers()
-> Result<()> {
    // Arrange
    let f = SocialTestFixture::new();
    let command_id = Uuid::new_v4();
    let follower_id = ProfileId::generate();
    let target_id = f.target_profile_id();

    f.given_existing_relation(follower_id, target_id);
    f.idempotency_repo().save(None, &command_id).await?;

    // 🚀 AJUSTEMENT RUST : Le champ region est retiré de la commande
    let cmd = UnfollowCommand {
        command_id,
        follower_id,
        target: CommandTarget::stateless(target_id),
        region: f.region(),
    };

    // Act
    f.bus()
        .execute::<SocialCommandCtx, UnfollowCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.relation_repo()
        .assert_relation_exists(follower_id, target_id)
        .await;

    f.relation_repo().assert_no_events_for(follower_id).await;

    Ok(())
}

#[tokio::test]
async fn test_unfollow_handler_should_ignore_self_unfollowing_attempts() -> Result<()> {
    // Arrange
    let f = SocialTestFixture::new();
    let actor_id = f.target_profile_id();

    // 🚀 AJUSTEMENT RUST : Le champ region est retiré de la commande
    let cmd = UnfollowCommand {
        command_id: Uuid::new_v4(),
        follower_id: actor_id,
        target: CommandTarget::stateless(actor_id),
        region: f.region(),
    };

    // Act
    f.bus()
        .execute::<SocialCommandCtx, UnfollowCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.relation_repo()
        .assert_relation_does_not_exist(actor_id, actor_id)
        .await;

    f.relation_repo().assert_no_events_for(actor_id).await;

    Ok(())
}

#[tokio::test]
async fn test_unfollow_handler_should_skip_execution_if_not_following() -> Result<()> {
    // Arrange
    let f = SocialTestFixture::new();
    let command_id = Uuid::new_v4();
    let follower_id = ProfileId::generate();
    let target_id = f.target_profile_id();

    f.given_initial_counters(follower_id, 0, 0);
    f.given_initial_counters(target_id, 0, 0);

    // 🚀 AJUSTEMENT RUST : Le champ region est retiré de la commande
    let cmd = UnfollowCommand {
        command_id,
        follower_id,
        target: CommandTarget::stateless(target_id),
        region: f.region(),
    };

    // Act
    f.bus()
        .execute::<SocialCommandCtx, UnfollowCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    assert!(
        f.idempotency_repo().exists(None, &command_id).await?,
        "Le CommandBus aurait dû marquer la commande comme traitée"
    );

    f.cache_counter_repo()
        .assert_counters_values(follower_id, 0, 0)
        .await;

    f.relation_repo().assert_no_events_for(follower_id).await;

    Ok(())
}

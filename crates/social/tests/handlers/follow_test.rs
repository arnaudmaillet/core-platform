use shared_kernel::command::CommandTarget;
use shared_kernel::core::Result;
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::ProfileId;
use social::commands::FollowCommand;
use social::context::SocialCommandContext;
use social::repositories::CounterRepository;
use social_test_utils::SocialTestFixture;
use uuid::Uuid;

#[tokio::test]
async fn test_follow_handler_success_nominal_path() -> Result<()> {
    // Arrange
    let f = SocialTestFixture::new();
    let follower_id = ProfileId::generate();
    let target_id = f.target_profile_id();

    let cmd = FollowCommand {
        command_id: Uuid::new_v4(),
        follower_id,
        target: CommandTarget::stateless(target_id),
        region: f.region(),
    };

    // Act
    f.bus()
        .execute::<SocialCommandContext, FollowCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // 1. Le graphe de relation est bien écrit en synchrone dans ScyllaDB
    f.assert_relation_exists(follower_id, target_id).await;

    // 2. Le Hot Path des compteurs est incrémenté et marqué DIRTY dans Redis
    f.assert_counters_values(follower_id, 0, 1).await; // Le follower suit 1 profil (+1 following)
    f.assert_counters_values(target_id, 1, 0).await; // La cible gagne 1 follower (+1 follower)

    Ok(())
}

#[tokio::test]
async fn test_follow_handler_should_abort_silently_when_idempotency_barrier_triggers() -> Result<()>
{
    // Arrange
    let f = SocialTestFixture::new();
    let command_id = Uuid::new_v4();
    let follower_id = ProfileId::generate();
    let target_id = f.target_profile_id();

    // On simule une commande déjà enregistrée à chaud (Idempotence Redis SET NX)
    f.idempotency_repo().save(None, &command_id).await?;

    let cmd = FollowCommand {
        command_id,
        follower_id,
        target: CommandTarget::stateless(target_id),
        region: f.region(),
    };

    // Act
    f.bus()
        .execute::<SocialCommandContext, FollowCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // L'idempotence technique a bloqué le flux : aucune écriture ne doit avoir eu lieu
    f.assert_relation_does_not_exist(follower_id, target_id)
        .await;

    Ok(())
}

#[tokio::test]
async fn test_follow_handler_should_ignore_self_following_attempts() -> Result<()> {
    // Arrange
    let f = SocialTestFixture::new();
    let actor_id = f.target_profile_id();

    // Invariant : follower_id == target_id (interdit)
    let cmd = FollowCommand {
        command_id: Uuid::new_v4(),
        follower_id: actor_id,
        target: CommandTarget::stateless(actor_id),
        region: f.region(),
    };

    // Act
    f.bus()
        .execute::<SocialCommandContext, FollowCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.assert_relation_does_not_exist(actor_id, actor_id).await;

    Ok(())
}

#[tokio::test]
async fn test_follow_handler_should_skip_execution_if_already_following() -> Result<()> {
    // Arrange
    let f = SocialTestFixture::new();
    let command_id = Uuid::new_v4();
    let follower_id = ProfileId::generate();
    let target_id = f.target_profile_id();

    // Business Idempotency : La relation existe déjà dans la base
    f.given_existing_relation(follower_id, target_id);
    f.given_initial_counters(follower_id, 0, 1);
    f.given_initial_counters(target_id, 1, 0);

    let cmd = FollowCommand {
        command_id,
        follower_id,
        target: CommandTarget::stateless(target_id),
        region: f.region(),
    };

    // Act
    f.bus()
        .execute::<SocialCommandContext, FollowCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // L'idempotence technique Redis ne doit pas avoir été posée car on a skip dès le début du Handler
    assert!(!f.idempotency_repo().exists(None, &command_id).await?);

    // Les compteurs n'ont pas bougé (pas de double incrémentation accidentelle)
    let redis_follower = f.cache_counter_repo().get_counters(follower_id).await?;
    assert_eq!(redis_follower.following_count().value(), 1);

    Ok(())
}

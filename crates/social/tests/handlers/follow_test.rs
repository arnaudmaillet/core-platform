use shared_kernel::command::CommandTarget;
use shared_kernel::core::Result;
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::ProfileId;
use social::commands::FollowCommand;
use social::context::SocialCommandContext;
use social::events::SocialEvent;
use social_test_utils::SocialTestFixture;
use social_test_utils::assertions::{CounterRepositoryAsserts, RelationRepositoryAsserts};
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
    f.relation_repo()
        .assert_relation_exists(follower_id, target_id)
        .await;

    // 2. Le Hot Path des compteurs est incrémenté et marqué DIRTY dans Redis
    f.cache_counter_repo()
        .assert_counters_values(follower_id, 0, 1)
        .await; // Le follower suit 1 profil (+1 following)
    f.cache_counter_repo()
        .assert_counters_values(target_id, 1, 0)
        .await; // La cible gagne 1 follower (+1 follower)
    f.cache_counter_repo().assert_profile_is_dirty(&follower_id);

    // 3. 💡 VÉRIFICATION DE L'ÉVÉNEMENT DU DOMAINE (Indexé par follower_id)
    f.relation_repo()
        .assert_captured_event_for(follower_id, |event| match event {
            SocialEvent::ProfileFollowed {
                follower_id: f_id,
                following_id: tg_id,
                ..
            } => {
                assert_eq!(
                    *f_id, follower_id,
                    "L'ID du follower dans l'événement est incorrect"
                );
                assert_eq!(
                    *tg_id, target_id,
                    "L'ID du following dans l'événement est incorrect"
                );
            }
            _ => panic!("Type d'événement incorrect capturé : attendu ProfileFollowed"),
        })
        .await;

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
    f.relation_repo()
        .assert_relation_does_not_exist(follower_id, target_id)
        .await;

    // Aucun événement ne doit avoir été déclenché pour ce follower_id
    f.relation_repo().assert_no_events_for(follower_id).await;

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
    f.relation_repo()
        .assert_relation_does_not_exist(actor_id, actor_id)
        .await;

    // 💡 La validation de l'invariant doit empêcher la levée d'événements
    f.relation_repo().assert_no_events_for(actor_id).await;

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
    // Le CommandBus centralisé DOIT avoir enregistré l'idempotence
    assert!(
        f.idempotency_repo().exists(None, &command_id).await?,
        "Le CommandBus aurait dû marquer la commande comme traitée"
    );

    // Les compteurs n'ont pas bougé (pas de double incrémentation accidentelle)
    f.cache_counter_repo()
        .assert_counters_values(follower_id, 0, 1)
        .await;

    // Comme le handler fait un skip précoce (early exit), aucun nouvel événement n'est généré
    f.relation_repo().assert_no_events_for(follower_id).await;

    Ok(())
}

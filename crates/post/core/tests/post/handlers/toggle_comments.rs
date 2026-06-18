// crates/post/core/tests/post/handlers/toggle_comments.rs

use post::PostCommandCtx;
use post::ToggleCommentsCommand;
use post_utils::{PostRepositoryAsserts, PostTestFixture};
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, ManagedEntity, Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use uuid::Uuid;

#[tokio::test]
async fn test_toggle_comments_handler_success() -> Result<()> {
    // Arrange
    let f = PostTestFixture::new();
    let post = f.builder("Some caption").build().unwrap();
    let version_snapshot = post.version();

    f.given_post(&post).await;
    let command_id = Uuid::new_v4();

    // On part de 'true' par défaut, on désactive via le contrôle de version OCC
    let cmd = ToggleCommentsCommand {
        command_id,
        target: CommandTarget::versioned(f.post_id(), version_snapshot),
        region: f.server_region(),
        allowed: false,
    };

    // Act
    f.bus()
        .execute::<PostCommandCtx, ToggleCommentsCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert : Vérification via la passerelle d'assertion
    f.post_assertions()
        .assert_post_state(f.post_id(), |p| {
            assert!(!p.allowed_comment_hands());
            assert_eq!(p.version(), version_snapshot + 1); // Incrémentation OCC
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_toggle_comments_handler_business_idempotency() -> Result<()> {
    // Arrange
    let f = PostTestFixture::new();
    let post = f.builder("Some caption").build().unwrap();
    let version_snapshot = post.version();

    f.given_post(&post).await;
    let command_id = Uuid::new_v4();

    let cmd = ToggleCommentsCommand {
        command_id,
        target: CommandTarget::versioned(f.post_id(), version_snapshot),
        region: f.server_region(),
        allowed: true,
    };

    // Act
    f.bus()
        .execute::<PostCommandCtx, ToggleCommentsCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert : Rien ne doit muter, la version et la date de mise à jour restent stables
    f.post_assertions()
        .assert_post_state(f.post_id(), |p| {
            assert!(p.allowed_comment_hands());
            assert_eq!(p.version(), version_snapshot);
            assert_eq!(p.lifecycle().updated_at(), post.lifecycle().updated_at());
        })
        .await;

    Ok(())
}

// #[tokio::test]
// async fn test_toggle_comments_handler_technical_idempotency_barrier() -> Result<()> {
//     // Arrange
//     let f = PostTestFixture::new();
//     let post = f.builder("Some caption").build().unwrap();
//     let version_snapshot = post.version();

//     f.given_post(&post).await;
//     let command_id = Uuid::new_v4();

//     // On simule une commande déjà exécutée et enregistrée au premier rideau
//     f.idempotency_repo().save(None, &command_id).await?;

//     let cmd = ToggleCommentsCommand {
//         command_id,
//         target: CommandTarget::versioned(f.post_id(), version_snapshot),
//         region: f.server_region(),
//         allowed: false,
//     };

//     // Act
//     f.bus()
//         .execute::<PostCommandCtx, ToggleCommentsCommand, ()>(f.command_ctx().clone(), cmd)
//         .await?;

//     // Assert : Bloqué par la barrière technique Redis, l'état reste inchangé (true)
//     f.post_assertions()
//         .assert_post_state(f.post_id(), |p| {
//             assert!(p.allowed_comment_hands());
//             assert_eq!(p.version(), version_snapshot);
//         })
//         .await;

//     Ok(())
// }

#[tokio::test]
async fn test_toggle_comments_concurrency_conflict() -> Result<()> {
    // Arrange
    let f = PostTestFixture::new();
    let post = f.builder("Some caption").build().unwrap();
    f.given_post(&post).await;

    // Concurrence conflict simulation (Mauvaise version fournie)
    let cmd = ToggleCommentsCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.post_id(), 99),
        region: f.server_region(),
        allowed: false,
    };

    // Act
    let result = f
        .bus()
        .execute::<PostCommandCtx, ToggleCommentsCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(matches!(
        result,
        Err(e) if e.code == ErrorCode::ConcurrencyConflict
    ));

    // L'état en base doit être resté intact
    f.post_assertions()
        .assert_post_state(f.post_id(), |p| {
            assert!(p.allowed_comment_hands());
            assert_eq!(p.version(), post.version());
        })
        .await;

    Ok(())
}

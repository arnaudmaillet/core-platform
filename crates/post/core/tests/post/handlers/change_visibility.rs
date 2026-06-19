// crates/post/core/tests/post/handlers/change_visibility.rs

use post::{ChangeVisibilityCommand, PostCommandCtx, VisibilityLevel};
use post_utils::{PostRepositoryAsserts, PostTestFixture};
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ManagedEntity, Result};
use uuid::Uuid;

#[tokio::test]
async fn test_change_visibility_handler_success() -> Result<()> {
    // Arrange
    let f = PostTestFixture::new();
    let post = f.builder("Some caption").build().unwrap();

    f.given_post(&post).await;
    let command_id = Uuid::new_v4();

    let cmd = ChangeVisibilityCommand {
        command_id,
        target: CommandTarget::stateless(f.post_id()),
        region: f.server_region(),
        new_visibility: VisibilityLevel::Private,
    };

    // Act
    f.bus()
        .execute::<PostCommandCtx, ChangeVisibilityCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.post_assertions()
        .assert_post_state(f.post_id(), |p| {
            assert_eq!(p.visibility_level(), VisibilityLevel::Private);
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_change_visibility_handler_business_idempotency() -> Result<()> {
    // Arrange
    let f = PostTestFixture::new();
    let post = f.builder("Public post").build().unwrap();

    f.given_post(&post).await;
    let command_id = Uuid::new_v4();

    // On envoie la même visibilité que celle d'origine (Public)
    let cmd = ChangeVisibilityCommand {
        command_id,
        target: CommandTarget::stateless(f.post_id()),
        region: f.server_region(),
        new_visibility: VisibilityLevel::Public,
    };

    // Act
    f.bus()
        .execute::<PostCommandCtx, ChangeVisibilityCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert : Aucun changement d'état métier n'a été provoqué, l'instant de modification reste inchangé
    f.post_assertions()
        .assert_post_state(f.post_id(), |p| {
            assert_eq!(p.visibility_level(), VisibilityLevel::Public);
            assert_eq!(p.lifecycle().updated_at(), post.lifecycle().updated_at());
        })
        .await;

    Ok(())
}

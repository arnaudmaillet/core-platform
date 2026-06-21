// crates/post/core/tests/post/handlers/toggle_comments.rs

use post_older::PostCommandCtx;
use post_older::ToggleCommentsCommand;
use post_utils::{PostRepositoryAsserts, PostTestFixture};
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ManagedEntity, Result};
use uuid::Uuid;

#[tokio::test]
async fn test_toggle_comments_handler_success() -> Result<()> {
    // Arrange
    let f = PostTestFixture::new();
    let post = f.builder("Some caption").build().unwrap();

    f.given_post(&post).await;
    let command_id = Uuid::new_v4();

    // On part de 'true' par défaut, on désactive via le contrôle de version OCC
    let cmd = ToggleCommentsCommand {
        command_id,
        target: CommandTarget::stateless(f.post_id()),
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
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_toggle_comments_handler_business_idempotency() -> Result<()> {
    // Arrange
    let f = PostTestFixture::new();
    let post = f.builder("Some caption").build().unwrap();

    f.given_post(&post).await;
    let command_id = Uuid::new_v4();

    let cmd = ToggleCommentsCommand {
        command_id,
        target: CommandTarget::stateless(f.post_id()),
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
            assert_eq!(p.lifecycle().updated_at(), post.lifecycle().updated_at());
        })
        .await;

    Ok(())
}

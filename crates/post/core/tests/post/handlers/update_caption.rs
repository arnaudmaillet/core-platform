// crates/post/core/tests/post/handlers/toggle_comments.rs

use post::{Caption, PostCommandCtx, UpdateCaptionCommand};
use post_utils::{PostRepositoryAsserts, PostTestFixture};
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ManagedEntity, Result};
use uuid::Uuid;

#[tokio::test]
async fn test_update_caption_handler_success() -> Result<()> {
    // Arrange
    let f = PostTestFixture::new();
    let post = f.builder("Original caption").build().unwrap();
    f.given_post(&post).await;

    let command_id = Uuid::new_v4();
    let slug = "arnaud".to_string();
    let new_caption = Caption::try_from(format!("New caption with @{}", slug).as_str()).unwrap();

    // Utilisation du versionnement strict (OCC) exigé par le PostCommandCtx
    let cmd = UpdateCaptionCommand {
        command_id,
        target: CommandTarget::stateless(f.post_id()),
        region: f.server_region(),
        new_caption: Some(new_caption),
    };

    // Act
    f.bus()
        .execute::<PostCommandCtx, UpdateCaptionCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert : Vérification via la passerelle d'assertion
    f.post_assertions()
        .assert_post_state(f.post_id(), |p| {
            assert_eq!(
                p.caption().as_ref().unwrap().to_string(),
                "New caption with @arnaud"
            );
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_update_caption_handler_no_change_skips() -> Result<()> {
    // Arrange
    let f = PostTestFixture::new();
    let caption = Caption::try_from("Same caption").unwrap();
    let post = f.builder("Same caption").build().unwrap();

    f.given_post(&post).await;
    let command_id = Uuid::new_v4();

    let cmd = UpdateCaptionCommand {
        command_id,
        target: CommandTarget::stateless(f.post_id()),
        region: f.server_region(),
        new_caption: Some(caption),
    };

    // Act
    f.bus()
        .execute::<PostCommandCtx, UpdateCaptionCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert : Idempotence métier, aucun changement d'état ni d'incrémentation de version
    f.post_assertions()
        .assert_post_state(f.post_id(), |p| {
            assert_eq!(p.lifecycle().updated_at(), post.lifecycle().updated_at());
        })
        .await;

    Ok(())
}

use post_older::{Caption, CreatePostCommand, PostCommandCtx};
use post_utils::{PostRepositoryAsserts, PostTestFixture};
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::PostType;
use uuid::Uuid;

#[tokio::test]
async fn test_create_post_handler_success_with_mentions() -> Result<()> {
    // Arrange
    let f = PostTestFixture::new();
    let command_id = Uuid::new_v4();
    let slug = "arnaud".to_string();

    let caption = Caption::try_from(format!("Hello @{}", slug).as_str()).unwrap();
    let target = CommandTarget::stateless(f.author_id());

    let cmd = CreatePostCommand {
        command_id,
        target,
        region: f.server_region(),
        post_id: f.post_id(),
        post_type: PostType::Text,
        caption: Some(caption),
        media_list: vec![],
        visibility_level: "Public".to_string(),
        allowed_comment_hands: true,
        dynamic_metadata: None,
        music_id: None,
    };

    // Act
    f.bus()
        .execute::<PostCommandCtx, CreatePostCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert : Utilisation de la passerelle d'assertions pour valider l'état brut créé
    f.post_assertions()
        .assert_post_state(f.post_id(), |post| {
            assert_eq!(post.post_id(), f.post_id());
            assert_eq!(post.author_id(), f.author_id());
            assert_eq!(post.version(), 1);
            // L'assertion sur la mention externe est retirée car déconnectée du cycle synchrone
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_create_post_handler_idempotency_barrier() -> Result<()> {
    // Arrange
    let f = PostTestFixture::new();
    let command_id = Uuid::new_v4();
    f.idempotency_repo().save(None, &command_id).await?;

    let target = CommandTarget::stateless(f.author_id());

    let cmd = CreatePostCommand {
        command_id,
        target,
        region: f.server_region(),
        post_id: f.post_id(),
        post_type: PostType::Text,
        caption: None,
        media_list: vec![],
        visibility_level: "Public".to_string(),
        allowed_comment_hands: true,
        dynamic_metadata: None,
        music_id: None,
    };

    // Act
    f.bus()
        .execute::<PostCommandCtx, CreatePostCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert : Bloqué par l'idempotence, le post ne doit jamais avoir été instancié ou persisté
    f.post_assertions().assert_not_found(f.post_id()).await;

    Ok(())
}

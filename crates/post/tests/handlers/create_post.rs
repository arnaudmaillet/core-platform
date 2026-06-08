#[cfg(test)]
mod tests {
    use post::commands::CreatePostCommand;
    use post::context::PostCommandContext;
    use post::repositories::PostRepository;
    use post::types::Caption;
    use post_test_utils::PostTestFixture;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::Result;
    use shared_kernel::idempotency::IdempotencyRepository;
    use shared_kernel::types::{PostType, ProfileId};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_create_post_handler_success_with_mentions() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let command_id = Uuid::now_v7();
        let target_profile = ProfileId::generate();
        let slug = "arnaud".to_string();

        let mut map = BTreeMap::new();
        map.insert(slug.clone(), target_profile);
        f.profile_resolver().set_stub_map(map);

        let caption = Caption::try_from(format!("Hello @{}", slug).as_str()).unwrap();
        let target = CommandTarget::stateless(f.author_id());

        let cmd = CreatePostCommand {
            command_id,
            target,
            region: f.region(),
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
            .execute::<PostCommandContext, CreatePostCommand, ()>(f.writer_ctx(), cmd)
            .await?;

        // Assert
        let post = f
            .post_repo()
            .find_by_id(f.region(), &f.post_id())
            .await?
            .unwrap();
        assert!(post.mentions().contains(&target_profile));
        assert!(f.idempotency_repo().exists(None, &command_id).await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_create_post_handler_idempotency_barrier() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let command_id = Uuid::now_v7();
        f.idempotency_repo().save(None, &command_id).await?;

        let target = CommandTarget::stateless(f.author_id());

        let cmd = CreatePostCommand {
            command_id,
            target,
            region: f.region(),
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
            .execute::<PostCommandContext, CreatePostCommand, ()>(f.writer_ctx(), cmd)
            .await?;

        // Assert
        let post = f.post_repo().find_by_id(f.region(), &f.post_id()).await?;
        assert!(post.is_none());

        Ok(())
    }
}

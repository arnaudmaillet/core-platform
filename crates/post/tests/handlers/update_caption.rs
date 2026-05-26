#[cfg(test)]
mod tests {
    use post::commands::UpdateCaptionCommand;
    use post::context::PostCommandContext;
    use post::repositories::PostRepository;
    use post::types::Caption;
    use post_test_utils::PostTestFixture;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{Result, Versioned};
    use shared_kernel::idempotency::IdempotencyRepository;
    use shared_kernel::types::ProfileId;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_update_caption_handler_success() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Original caption").build().unwrap();
        f.given_post(&post).await;

        let command_id = Uuid::now_v7();
        let target_profile = ProfileId::generate(f.region());
        let slug = "arnaud".to_string();

        // Configurer le stub pour résoudre "@arnaud" vers target_profile
        let mut map = BTreeMap::new();
        map.insert(slug.clone(), target_profile);
        f.profile_resolver().set_stub_map(map);

        let new_caption =
            Caption::try_from(format!("New caption with @{}", slug).as_str()).unwrap();
        let cmd = UpdateCaptionCommand {
            command_id,
            target: CommandTarget::new(post.post_id(), f.region(), post.version()),
            new_caption: Some(new_caption),
        };

        // Act
        f.bus()
            .execute::<PostCommandContext, UpdateCaptionCommand, ()>(f.writer_ctx(), cmd)
            .await?;

        // Assert
        let updated_post = f
            .post_repo()
            .find_by_id(f.region(), &post.post_id())
            .await?
            .unwrap();
        assert_eq!(
            updated_post.caption().as_ref().unwrap().to_string(),
            "New caption with @arnaud"
        );
        assert!(updated_post.mentions().contains(&target_profile));
        assert!(f.idempotency_repo().exists(None, &command_id).await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_caption_handler_no_change_skips() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let caption = Caption::try_from("Same caption").unwrap();
        let post = f.builder("Same caption").build().unwrap();
        f.given_post(&post).await;
        let command_id = Uuid::now_v7();

        let cmd = UpdateCaptionCommand {
            command_id,
            target: CommandTarget::new(post.post_id(), f.region(), post.version()),
            new_caption: Some(caption),
        };

        // Act
        f.bus()
            .execute::<PostCommandContext, UpdateCaptionCommand, ()>(f.writer_ctx(), cmd)
            .await?;

        // Assert
        assert!(!f.idempotency_repo().exists(None, &command_id).await?);

        Ok(())
    }
}

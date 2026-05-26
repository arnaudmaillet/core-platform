#[cfg(test)]
mod tests {
    use post::commands::ChangeVisibilityCommand;
    use post::context::PostCommandContext;
    use post::repositories::PostRepository;
    use post::types::VisibilityLevel;
    use post_test_utils::PostTestFixture;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{Result, Versioned};
    use shared_kernel::idempotency::IdempotencyRepository;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_change_visibility_handler_success() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Some caption").build().unwrap();
        f.given_post(&post).await;
        let command_id = Uuid::now_v7();

        let cmd = ChangeVisibilityCommand {
            command_id,
            target: CommandTarget::new(post.post_id(), f.region(), post.version()),
            new_visibility: VisibilityLevel::Private,
        };

        // Act
        f.bus()
            .execute::<PostCommandContext, ChangeVisibilityCommand, ()>(f.writer_ctx(), cmd)
            .await?;

        // Assert
        let updated_post = f
            .post_repo()
            .find_by_id(f.region(), &post.post_id())
            .await?
            .unwrap();
        assert_eq!(updated_post.visibility_level(), VisibilityLevel::Private);

        assert!(f.idempotency_repo().exists(None, &command_id).await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_change_visibility_handler_skips_when_already_set() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Public post").build().unwrap();
        f.given_post(&post).await;
        let command_id = Uuid::now_v7();

        let cmd = ChangeVisibilityCommand {
            command_id,
            target: CommandTarget::new(post.post_id(), f.region(), post.version()),
            new_visibility: VisibilityLevel::Public,
        };

        // Act
        f.bus()
            .execute::<PostCommandContext, ChangeVisibilityCommand, ()>(f.writer_ctx(), cmd)
            .await?;

        // Assert
        assert!(!f.idempotency_repo().exists(None, &command_id).await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_change_visibility_handler_idempotency_barrier() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Public post").build().unwrap();
        f.given_post(&post).await;
        let command_id = Uuid::now_v7();

        f.idempotency_repo().save(None, &command_id).await?;

        let cmd = ChangeVisibilityCommand {
            command_id,
            target: CommandTarget::new(post.post_id(), f.region(), post.version()),
            new_visibility: VisibilityLevel::Private,
        };

        // Act
        f.bus()
            .execute::<PostCommandContext, ChangeVisibilityCommand, ()>(f.writer_ctx(), cmd)
            .await?;

        // Assert
        let current_post = f
            .post_repo()
            .find_by_id(f.region(), &post.post_id())
            .await?
            .unwrap();

        assert_eq!(current_post.visibility_level(), VisibilityLevel::Public);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use post::commands::ToggleCommentsCommand;
    use post::context::PostCommandContext;
    use post::repositories::PostRepository;
    use post_test_utils::PostTestFixture;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{Result, Versioned};
    use shared_kernel::idempotency::IdempotencyRepository;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_toggle_comments_handler_success() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Some caption").build().unwrap();
        f.given_post(&post).await;
        let command_id = Uuid::now_v7();

        // On part de 'true' par défaut, on désactive
        let cmd = ToggleCommentsCommand {
            command_id,
            target: CommandTarget::new(post.post_id(), f.region(), post.version()),
            allowed: false,
        };

        // Act
        f.bus()
            .execute::<PostCommandContext, ToggleCommentsCommand, ()>(f.writer_ctx(), cmd)
            .await?;

        // Assert
        let updated_post = f
            .post_repo()
            .find_by_id(f.region(), &post.post_id())
            .await?
            .unwrap();
        assert!(!updated_post.allowed_comment_hands());

        assert!(f.idempotency_repo().exists(None, &command_id).await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_toggle_comments_handler_skips_when_already_set() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Some caption").build().unwrap();
        f.given_post(&post).await;
        let command_id = Uuid::now_v7();

        // Déjà 'true' par défaut, on tente de mettre 'true'
        let cmd = ToggleCommentsCommand {
            command_id,
            target: CommandTarget::new(post.post_id(), f.region(), post.version()),
            allowed: true,
        };

        // Act
        f.bus()
            .execute::<PostCommandContext, ToggleCommentsCommand, ()>(f.writer_ctx(), cmd)
            .await?;

        // Assert
        assert!(!f.idempotency_repo().exists(None, &command_id).await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_toggle_comments_handler_idempotency_barrier() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Some caption").build().unwrap();
        f.given_post(&post).await;
        let command_id = Uuid::now_v7();

        f.idempotency_repo().save(None, &command_id).await?;

        let cmd = ToggleCommentsCommand {
            command_id,
            target: CommandTarget::new(post.post_id(), f.region(), post.version()),
            allowed: false,
        };

        // Act
        f.bus()
            .execute::<PostCommandContext, ToggleCommentsCommand, ()>(f.writer_ctx(), cmd)
            .await?;

        // Assert
        let current_post = f
            .post_repo()
            .find_by_id(f.region(), &post.post_id())
            .await?
            .unwrap();

        // Rien ne doit avoir changé (toujours true par défaut)
        assert!(current_post.allowed_comment_hands());

        Ok(())
    }
}

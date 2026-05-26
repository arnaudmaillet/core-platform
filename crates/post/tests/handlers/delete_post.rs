#[cfg(test)]
mod tests {
    use post::commands::DeletePostCommand;
    use post::context::PostCommandContext;
    use post::repositories::PostRepository;
    use post_test_utils::PostTestFixture;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{Result, Versioned};
    use shared_kernel::idempotency::IdempotencyRepository;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_delete_post_handler_success() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("To be deleted").build().unwrap();
        f.given_post(&post).await;
        let command_id = Uuid::now_v7();

        let cmd = DeletePostCommand {
            command_id,
            target: CommandTarget::new(post.post_id(), f.region(), post.version()),
        };

        // Act
        f.bus()
            .execute::<PostCommandContext, DeletePostCommand, ()>(f.writer_ctx(), cmd)
            .await?;

        // Assert
        // Vérifie que le post n'est plus accessible (ou marqué comme supprimé selon ton implémentation)
        let saved_post = f
            .post_repo()
            .find_by_id(f.region(), &post.post_id())
            .await?;
        assert!(saved_post.is_none()); // Ou vérifie un flag 'is_deleted' si c'est un soft-delete

        assert!(f.idempotency_repo().exists(None, &command_id).await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_post_handler_idempotency_barrier() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("To be deleted").build().unwrap();
        f.given_post(&post).await;
        let command_id = Uuid::now_v7();

        f.idempotency_repo().save(None, &command_id).await?;

        let cmd = DeletePostCommand {
            command_id,
            target: CommandTarget::new(post.post_id(), f.region(), post.version()),
        };

        // Act
        f.bus()
            .execute::<PostCommandContext, DeletePostCommand, ()>(f.writer_ctx(), cmd)
            .await?;

        // Assert
        // Le post doit toujours exister car la commande a été ignorée par la barrière
        let current_post = f
            .post_repo()
            .find_by_id(f.region(), &post.post_id())
            .await?;
        assert!(current_post.is_some());

        Ok(())
    }
}

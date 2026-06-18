// crates/post/src/application/commands/delete_post/tests.rs

#[cfg(test)]
mod tests {
    use post::commands::DeletePostCommand;
    use post::context::PostCommandCtx;
    use post_test_utils::PostTestFixture;
    use post_test_utils::assertions::PostRepositoryAsserts;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{ErrorCode, Result, Versioned};
    use shared_kernel::idempotency::IdempotencyRepository;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_delete_post_handler_success() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("To be deleted").build().unwrap();
        let version_snapshot = post.version();

        f.given_post(&post).await;
        let command_id = Uuid::new_v4();

        let cmd = DeletePostCommand {
            command_id,
            target: CommandTarget::versioned(f.post_id(), version_snapshot),
            region: f.server_region(),
        };

        // Act
        f.bus()
            .execute::<PostCommandCtx, DeletePostCommand, ()>(f.command_ctx().clone(), cmd)
            .await?;

        // Assert : Utilisation de la méthode dédiée du stub pour valider la purge physique en mémoire brute
        f.post_assertions().assert_not_found(f.post_id()).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_post_handler_technical_idempotency_barrier() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("To be deleted safely").build().unwrap();
        let version_snapshot = post.version();

        f.given_post(&post).await;
        let command_id = Uuid::new_v4();

        // On injecte manuellement le jeton d'idempotence technique en amont dans Redis/Stub
        f.idempotency_repo().save(None, &command_id).await?;

        let cmd = DeletePostCommand {
            command_id,
            target: CommandTarget::versioned(f.post_id(), version_snapshot),
            region: f.server_region(),
        };

        // Act
        f.bus()
            .execute::<PostCommandCtx, DeletePostCommand, ()>(f.command_ctx().clone(), cmd)
            .await?;

        // Assert : La barrière d'idempotence doit stopper net l'exécution. Le post doit être intact.
        f.post_assertions()
            .assert_post_state(f.post_id(), |p| {
                assert_eq!(p.version(), version_snapshot);
            })
            .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_post_concurrency_conflict() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Delete concurrency block").build().unwrap();
        f.given_post(&post).await;

        // Commande ciblant une version erronée (OCC conflict simulation)
        let cmd = DeletePostCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::versioned(f.post_id(), 99),
            region: f.server_region(),
        };

        // Act
        let result = f
            .bus()
            .execute::<PostCommandCtx, DeletePostCommand, ()>(f.command_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(e) if e.code == ErrorCode::ConcurrencyConflict
        ));

        // Le post ne doit surtout pas avoir été supprimé de la base
        f.post_assertions()
            .assert_post_state(f.post_id(), |p| {
                assert_eq!(p.version(), post.version());
            })
            .await;

        Ok(())
    }
}

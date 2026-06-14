// crates/post/src/application/commands/change_visibility/tests.rs

#[cfg(test)]
mod tests {
    use post::commands::ChangeVisibilityCommand;
    use post::context::PostCommandCtx;
    use post::types::VisibilityLevel;
    use post_test_utils::PostTestFixture;
    use post_test_utils::assertions::PostRepositoryAsserts;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{ErrorCode, ManagedEntity, Result, Versioned};
    use shared_kernel::idempotency::IdempotencyRepository;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_change_visibility_handler_success() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Some caption").build().unwrap();
        let version_snapshot = post.version();

        f.given_post(&post).await;
        let command_id = Uuid::new_v4();

        let cmd = ChangeVisibilityCommand {
            command_id,
            target: CommandTarget::versioned(f.post_id(), version_snapshot),
            region: f.server_region(),
            new_visibility: VisibilityLevel::Private,
        };

        // Act
        f.bus()
            .execute::<PostCommandCtx, ChangeVisibilityCommand, ()>(f.command_ctx().clone(), cmd)
            .await?;

        // Assert : Utilisation de la passerelle d'assertion sur la mémoire brute du stub
        f.post_assertions()
            .assert_post_state(f.post_id(), |p| {
                assert_eq!(p.visibility_level(), VisibilityLevel::Private);
                assert_eq!(p.version(), version_snapshot + 1); // La version doit s'incrémenter à la sauvegarde
            })
            .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_change_visibility_handler_business_idempotency() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Public post").build().unwrap();
        let version_snapshot = post.version();

        f.given_post(&post).await;
        let command_id = Uuid::new_v4();

        // On envoie la même visibilité que celle d'origine (Public)
        let cmd = ChangeVisibilityCommand {
            command_id,
            target: CommandTarget::versioned(f.post_id(), version_snapshot),
            region: f.server_region(),
            new_visibility: VisibilityLevel::Public,
        };

        // Act
        f.bus()
            .execute::<PostCommandCtx, ChangeVisibilityCommand, ()>(f.command_ctx().clone(), cmd)
            .await?;

        // Assert : Pas de modification d'état métier, la version et l'updated_at ne doivent pas bouger
        f.post_assertions()
            .assert_post_state(f.post_id(), |p| {
                assert_eq!(p.version(), version_snapshot);
                assert_eq!(p.lifecycle().updated_at(), post.lifecycle().updated_at());
            })
            .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_change_visibility_handler_technical_idempotency_barrier() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Public post").build().unwrap();
        let version_snapshot = post.version();

        f.given_post(&post).await;
        let command_id = Uuid::new_v4();

        f.idempotency_repo().save(None, &command_id).await?;

        let cmd = ChangeVisibilityCommand {
            command_id,
            target: CommandTarget::versioned(f.post_id(), version_snapshot),
            region: f.server_region(),
            new_visibility: VisibilityLevel::Private,
        };

        // Act
        f.bus()
            .execute::<PostCommandCtx, ChangeVisibilityCommand, ()>(f.command_ctx().clone(), cmd)
            .await?;

        // Assert : La barrière d'idempotence doit stopper net l'exécution. L'état reste inchangé.
        f.post_assertions()
            .assert_post_state(f.post_id(), |p| {
                assert_eq!(p.visibility_level(), VisibilityLevel::Public);
                assert_eq!(p.version(), version_snapshot);
            })
            .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_change_visibility_concurrency_conflict() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Concurrency post").build().unwrap();
        f.given_post(&post).await;

        // Commande ciblant une version erronée (OCC conflict simulation)
        let cmd = ChangeVisibilityCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::versioned(f.post_id(), 99), // Mauvaise version attendue
            region: f.server_region(),
            new_visibility: VisibilityLevel::Private,
        };

        // Act
        let result = f
            .bus()
            .execute::<PostCommandCtx, ChangeVisibilityCommand, ()>(f.command_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(e) if e.code == ErrorCode::ConcurrencyConflict
        ));

        // Le post en base de données doit être resté intact
        f.post_assertions()
            .assert_post_state(f.post_id(), |p| {
                assert_eq!(p.visibility_level(), VisibilityLevel::Public);
                assert_eq!(p.version(), post.version());
            })
            .await;

        Ok(())
    }
}

// crates/profile/src/application/commands/media/remove_avatar/remove_avatar_handler.rs

#[cfg(test)]
mod tests {
    use crate::application::utils::ProfileTestFixture;
    use crate::commands::RemoveAvatarCommand;
    use crate::context::ProfileContext;
    use crate::events::ProfileEvent;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{ErrorCode, Result, Versioned};
    use shared_kernel::types::Url;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_remove_avatar_success() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();

        // On crée un profil qui POSSÈDE un avatar
        let avatar_url = Url::try_new("https://cdn.test.com/avatar.png")?;
        let profile = f.builder("alice").with_avatar(avatar_url).build()?;

        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let cmd = RemoveAvatarCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id(), f.region(), version_snapshot),
        };

        // Act
        f.bus()
            .execute::<ProfileContext, RemoveAvatarCommand, ()>(f.profile_ctx().clone(), cmd)
            .await?;

        // Assert
        let _ = f.assert_profile(|p| {
            // On vérifie que l'avatar est bien devenu None
            assert!(p.avatar().is_none());
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

        f.assert_outbox(1, Some(ProfileEvent::AVATAR_REMOVED));

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_avatar_business_idempotency() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();

        // Le profil n'a DÉJÀ PAS d'avatar
        let profile = f.builder("alice").build()?;

        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let cmd = RemoveAvatarCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id(), f.region(), version_snapshot),
        };

        // Act
        f.bus()
            .execute::<ProfileContext, RemoveAvatarCommand, ()>(f.profile_ctx().clone(), cmd)
            .await?;

        // Assert
        let _ = f.assert_profile(|p| {
            assert_eq!(p.version(), version_snapshot); // Pas de save car pas de changement
        })
        .await;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_avatar_technical_idempotency() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let cmd_id = Uuid::new_v4();

        // 1. On simule que la commande a déjà été traitée avec succès par le passé
        f.idempotency_repo().seed(cmd_id);

        // 2. On crée un profil avec un avatar
        let profile = f
            .builder("alice")
            .with_avatar(Url::try_new("https://cdn.com/avatar.png")?)
            .build()?;
        f.given_profile(profile).await;

        let cmd = RemoveAvatarCommand {
            command_id: cmd_id, // Même ID que celui enregistré en "seed"
            target: CommandTarget::new(f.profile_id(), f.region(), 0),
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, RemoveAvatarCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        // On s'attend à une erreur AlreadyExists sur l'entité "Command"
        assert!(
            result.is_ok(),
            "L'idempotence technique doit être transparente (Ok)"
        );

        // On vérifie que l'avatar est toujours là (car la commande a été stoppée net)
        let _ = f.assert_profile(|p| {
            assert!(p.avatar().is_some());
        })
        .await;

        // Pas d'événement supplémentaire dans l'outbox
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_avatar_concurrency_conflict() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let profile = f.builder("alice").build()?;
        f.given_profile(profile).await;

        let cmd = RemoveAvatarCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id(), f.region(), 42), // Version erronée
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, RemoveAvatarCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(e) if e.code == ErrorCode::ConcurrencyConflict
        ));

        Ok(())
    }
}

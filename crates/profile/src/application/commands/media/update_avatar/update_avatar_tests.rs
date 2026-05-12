// crates/profile/src/application/commands/media/update_avatar/update_avatar_handler.rs

#[cfg(test)]
mod tests {
    use crate::application::utils::ProfileTestFixture;
    use crate::commands::UpdateAvatarCommand;
    use crate::context::ProfileContext;
    use crate::events::ProfileEvent;
    use shared_kernel::application::CommandTarget;
    use shared_kernel::domain::entities::Versioned;
    use shared_kernel::domain::value_objects::Url;
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_update_avatar_success() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();

        // Profil sans avatar au départ
        let profile = f.builder("alice").build()?;
        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let new_url = Url::try_new("https://cdn.test.com/new_avatar.png")?;

        let cmd = UpdateAvatarCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id().clone(), f.region(), version_snapshot),
            new_avatar_url: new_url.clone(),
        };

        // Act
        f.bus()
            .execute::<ProfileContext, UpdateAvatarCommand, ()>(f.profile_ctx().clone(), cmd)
            .await?;

        // Assert
        f.assert_profile(|p| {
            assert_eq!(p.avatar(), Some(&new_url));
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

        f.assert_outbox(1, Some(ProfileEvent::AVATAR_UPDATED));

        Ok(())
    }

    #[tokio::test]
    async fn test_update_avatar_technical_idempotency() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let cmd_id = Uuid::new_v4();

        // 1. On "seed" le repo d'idempotence pour simuler que cette commande a déjà été traitée
        f.idempotency_repo().seed(cmd_id);

        let profile = f.builder("alice").build()?;
        f.given_profile(profile).await;

        let cmd = UpdateAvatarCommand {
            command_id: cmd_id, // Même ID que celui seedé
            target: CommandTarget::new(f.profile_id().clone(), f.region(), 0),
            new_avatar_url: Url::try_new("https://cdn.test.com/new.png")?,
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, UpdateAvatarCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        // Doit retourner une erreur AlreadyExists (Command id)
        assert!(
            matches!(result, Err(DomainError::AlreadyExists { entity, .. }) if entity == "Command")
        );

        // On vérifie que rien n'a été émis
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_avatar_business_idempotency() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let current_url = Url::try_new("https://cdn.test.com/existing.png")?;

        // Le profil a déjà cet avatar
        let profile = f
            .builder("alice")
            .with_avatar(current_url.clone())
            .build()?;

        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let cmd = UpdateAvatarCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id().clone(), f.region(), version_snapshot),
            new_avatar_url: current_url, // Même URL
        };

        // Act
        f.bus()
            .execute::<ProfileContext, UpdateAvatarCommand, ()>(f.profile_ctx().clone(), cmd)
            .await?;

        // Assert
        f.assert_profile(|p| {
            assert_eq!(p.version(), version_snapshot); // Pas de changement de version
        })
        .await;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_avatar_conflict() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let profile = f.builder("alice").build()?;
        f.given_profile(profile).await;

        let cmd = UpdateAvatarCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id().clone(), f.region(), 10), // Mauvaise version
            new_avatar_url: Url::try_new("https://cdn.test.com/fail.png")?,
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, UpdateAvatarCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));

        Ok(())
    }
}

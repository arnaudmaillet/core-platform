// crates/profile/src/application/commands/identity/update_display_name_handler.rs

#[cfg(test)]
mod tests {
    use crate::application::utils::ProfileTestFixture;
    use crate::commands::UpdateDisplayNameCommand;
    use crate::context::ProfileContext;
    use crate::events::ProfileEvent;
    use crate::value_objects::DisplayName;
    use shared_kernel::application::CommandTarget;
    use shared_kernel::core::{Error, ErrorCode, Result};
    use shared_kernel::domain::entities::Versioned;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_update_display_name_success() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        // On crée un profil existant
        let profile = f.builder("alice").build()?;
        let new_name = DisplayName::try_new("new_name")?;
        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let cmd = UpdateDisplayNameCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id().clone(), f.region(), version_snapshot),
            new_display_name: new_name.clone(),
        };

        // Act
        f.bus()
            .execute::<ProfileContext, UpdateDisplayNameCommand, ()>(f.profile_ctx().clone(), cmd)
            .await?;

        // Assert
        f.assert_profile(|p| {
            assert_eq!(p.display_name(), &new_name);
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

        // On vérifie qu'un événement est parti dans l'outbox
        f.assert_outbox(1, Some(ProfileEvent::DISPLAY_NAME_UPDATED));

        Ok(())
    }

    #[tokio::test]
    async fn test_update_display_name_technical_idempotency() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let cmd_id = Uuid::new_v4();

        // On simule que la commande a déjà été traitée (Idempotency Repo)
        f.idempotency_repo().seed(cmd_id);

        let profile = f.builder("Original").build()?;
        f.profile_repo().save_direct(profile).await;

        let cmd = UpdateDisplayNameCommand {
            command_id: cmd_id, // Même ID que seedé
            target: CommandTarget::new(f.profile_id().clone(), f.region(), 0),
            new_display_name: DisplayName::try_new("New Name")?,
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, UpdateDisplayNameCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(e) if e.code == ErrorCode::AlreadyExists
        ));
        f.assert_outbox(0, None); // Rien ne doit sortir

        Ok(())
    }

    #[tokio::test]
    async fn test_update_display_name_business_idempotency() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let name = DisplayName::try_new("Consistent Name")?;

        // Le profil a déjà le nom qu'on essaie de lui donner
        let profile = f.builder("alice").with_display_name(name.clone()).build()?;
        let version_snapshot = profile.version();
        f.profile_repo().save_direct(profile).await;

        let cmd = UpdateDisplayNameCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id().clone(), f.region(), version_snapshot),
            new_display_name: name,
        };

        // Act
        f.bus()
            .execute::<ProfileContext, UpdateDisplayNameCommand, ()>(f.profile_ctx().clone(), cmd)
            .await?;

        // Assert
        f.assert_profile(|p| {
            assert_eq!(p.version(), version_snapshot); // La version ne doit PAS bouger
        })
        .await;

        // Pas d'événement car pas de changement réel
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_display_name_conflict() -> Result<()> {
        let f = ProfileTestFixture::new();
        let profile = f.builder("alice").build()?;
        f.given_profile(profile).await;

        let cmd = UpdateDisplayNameCommand {
            command_id: Uuid::new_v4(),
            // On envoie une version attendue de 5 alors que le profil est en version 0
            target: CommandTarget::new(f.profile_id().clone(), f.region(), 5),
            new_display_name: DisplayName::try_new("wont_work")?,
        };

        let result = f
            .bus()
            .execute::<ProfileContext, UpdateDisplayNameCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Doit échouer avec une ConcurrencyConflict (levée par fetch_verified dans le handler)
        assert!(matches!(
            result,
            Err(e) if e.code == ErrorCode::ConcurrencyConflict
        ));

        Ok(())
    }
}

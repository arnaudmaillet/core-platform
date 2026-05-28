// crates/profile/src/application/commands/identity/update_privacy/update_privacy_handler.rs

#[cfg(test)]
mod tests {
    use crate::application::utils::ProfileTestFixture;
    use crate::commands::UpdatePrivacyCommand;
    use crate::context::ProfileCommandContext;
    use crate::events::ProfileEvent;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{ErrorCode, Result, Versioned};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_update_privacy_success() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();

        // On part d'un profil public (is_private: false par défaut dans le builder)
        let profile = f.builder("alice")?.with_privacy(false).build()?;
        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let cmd = UpdatePrivacyCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id(), f.region(), version_snapshot),
            is_private: true, // On passe en privé
        };

        // Act
        f.bus()
            .execute::<ProfileCommandContext, UpdatePrivacyCommand, ()>(
                f.command_ctx().clone(),
                cmd,
            )
            .await?;

        // Assert
        let _ = f
            .assert_profile(|p| {
                assert!(p.is_private());
                assert_eq!(p.version(), version_snapshot + 1);
            })
            .await;

        f.assert_outbox(1, Some(ProfileEvent::PRIVACY_UPDATED));

        Ok(())
    }

    #[tokio::test]
    async fn test_update_privacy_business_idempotency() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();

        // Le profil est déjà en privé
        let profile = f.builder("alice")?.with_privacy(true).build()?;
        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let cmd = UpdatePrivacyCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id(), f.region(), version_snapshot),
            is_private: true, // On demande encore du privé
        };

        // Act
        f.bus()
            .execute::<ProfileCommandContext, UpdatePrivacyCommand, ()>(
                f.command_ctx().clone(),
                cmd,
            )
            .await?;

        // Assert
        let _ = f
            .assert_profile(|p| {
                assert_eq!(p.version(), version_snapshot); // Pas d'incrément
            })
            .await;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_privacy_concurrency_conflict() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let profile = f.builder("alice")?.build()?;
        f.given_profile(profile).await;

        let cmd = UpdatePrivacyCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id(), f.region(), 99), // Mauvaise version attendue
            is_private: true,
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileCommandContext, UpdatePrivacyCommand, ()>(
                f.command_ctx().clone(),
                cmd,
            )
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(e) if e.code == ErrorCode::ConcurrencyConflict
        ));

        Ok(())
    }
}

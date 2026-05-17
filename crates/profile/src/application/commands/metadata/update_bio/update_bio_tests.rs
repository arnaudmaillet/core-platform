// crates/profile/src/application/commands/metadata/update_bio/update_bio_handler.rs

#[cfg(test)]
mod tests {
    use crate::application::utils::ProfileTestFixture;
    use crate::commands::UpdateBioCommand;
    use crate::context::ProfileContext;
    use crate::events::ProfileEvent;
    use crate::types::Bio;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{ErrorCode, Result, Versioned};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_update_bio_success() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let profile = f.builder("alice").build()?;
        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let new_bio = Some(Bio::try_new("Software Engineer & Rustacean")?);

        let cmd = UpdateBioCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id(), f.region(), version_snapshot),
            new_bio: new_bio.clone(),
        };

        // Act
        f.bus()
            .execute::<ProfileContext, UpdateBioCommand, ()>(f.profile_ctx().clone(), cmd)
            .await?;

        // Assert
        f.assert_profile(|p| {
            assert_eq!(p.bio(), new_bio.as_ref());
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

        f.assert_outbox(1, Some(ProfileEvent::BIO_UPDATED));

        Ok(())
    }

    #[tokio::test]
    async fn test_update_bio_technical_idempotency() -> Result<()> {
        // 1. Arrange
        let f = ProfileTestFixture::new();
        let cmd_id = Uuid::new_v4();

        f.idempotency_repo().seed(cmd_id);

        let profile = f.builder("alice").build()?;
        let initial_version = profile.version();
        f.given_profile(profile).await;

        let cmd = UpdateBioCommand {
            command_id: cmd_id,
            target: CommandTarget::new(f.profile_id(), f.region(), initial_version),
            new_bio: Some(Bio::try_new("Duplicate bio")?),
        };

        let result = f
            .bus()
            .execute::<ProfileContext, UpdateBioCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        assert!(
            result.is_ok(),
            "L'idempotence technique devrait être gérée comme un succès transparent"
        );

        f.assert_outbox(0, None);

        f.assert_profile(|p| {
            assert_eq!(
                p.version(),
                initial_version,
                "La version ne doit pas avoir augmenté"
            );
        })
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_update_bio_business_idempotency() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let bio_text = "Already my bio";
        let bio = Bio::try_new(bio_text)?;

        let profile = f.builder("alice").with_bio(bio.clone()).build()?;
        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let cmd = UpdateBioCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id(), f.region(), version_snapshot),
            new_bio: Some(bio), // Même contenu
        };

        // Act
        f.bus()
            .execute::<ProfileContext, UpdateBioCommand, ()>(f.profile_ctx().clone(), cmd)
            .await?;

        // Assert
        f.assert_profile(|p| {
            assert_eq!(p.version(), version_snapshot);
        })
        .await;
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_bio_concurrency_conflict() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let profile = f.builder("alice").build()?;
        f.given_profile(profile).await;

        let cmd = UpdateBioCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id(), f.region(), 42), // Version obsolète
            new_bio: Some(Bio::try_new("Conflict bio")?),
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, UpdateBioCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(e) if e.code == ErrorCode::ConcurrencyConflict
        ));

        Ok(())
    }
}

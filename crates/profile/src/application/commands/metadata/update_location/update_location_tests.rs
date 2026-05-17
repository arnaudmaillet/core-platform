// crates/profile/src/application/commands/metadata/update_location_label/update_location_label_handler.rs

#[cfg(test)]
mod tests {
    use crate::application::utils::ProfileTestFixture;
    use crate::commands::UpdateLocationCommand;
    use crate::context::ProfileContext;
    use crate::events::ProfileEvent;
    use crate::types::Location;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{ErrorCode, Result, Versioned};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_update_location_success() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let profile = f.builder("alice").build()?;
        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let new_location = Some(Location::try_new("Paris, France")?);

        let cmd = UpdateLocationCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id(), f.region(), version_snapshot),
            new_location: new_location.clone(),
        };

        // Act
        f.bus()
            .execute::<ProfileContext, UpdateLocationCommand, ()>(f.profile_ctx().clone(), cmd)
            .await?;

        // Assert
        f.assert_profile(|p| {
            assert_eq!(p.location(), new_location.as_ref());
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

        f.assert_outbox(1, Some(ProfileEvent::LOCATION_UPDATED));

        Ok(())
    }

    #[tokio::test]
    async fn test_update_location_technical_idempotency() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let cmd_id = Uuid::new_v4();
        f.idempotency_repo().seed(cmd_id);

        let profile = f.builder("alice").build()?;
        f.given_profile(profile).await;

        let cmd = UpdateLocationCommand {
            command_id: cmd_id,
            target: CommandTarget::new(f.profile_id(), f.region(), 0),
            new_location: Some(Location::try_new("Tokyo, Japan")?),
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, UpdateLocationCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(
            result.is_ok(),
            "L'idempotence technique doit être transparente (Ok)"
        );
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_location_business_idempotency() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let location = Location::try_new("Montreal, Canada")?;

        let profile = f.builder("alice").with_location(location.clone()).build()?;
        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let cmd = UpdateLocationCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id(), f.region(), version_snapshot),
            new_location: Some(location),
        };

        // Act
        f.bus()
            .execute::<ProfileContext, UpdateLocationCommand, ()>(f.profile_ctx().clone(), cmd)
            .await?;

        // Assert
        f.assert_profile(|p| {
            assert_eq!(p.version(), version_snapshot); // Pas de changement
        })
        .await;
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_location_concurrency_conflict() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let profile = f.builder("alice").build()?;
        f.given_profile(profile).await;

        let cmd = UpdateLocationCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id(), f.region(), 123), // Version dans le futur
            new_location: Some(Location::try_new("Nowhere")?),
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, UpdateLocationCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(e) if e.code == ErrorCode::ConcurrencyConflict
        ));

        Ok(())
    }
}

// crates/profile/src/application/commands/media/update_banner/update_banner_handler.rs

#[cfg(test)]
mod tests {
    use crate::application::utils::ProfileTestFixture;
    use crate::commands::UpdateBannerCommand;
    use crate::context::ProfileContext;
    use crate::events::ProfileEvent;
    use shared_kernel::application::CommandTarget;
    use shared_kernel::core::{ErrorCode, Result, Versioned};
    use shared_kernel::types::Url;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_update_banner_success() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let profile = f.builder("alice").build()?; // Pas de bannière au début
        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let new_url = Url::try_new("https://cdn.test.com/new_banner.png")?;

        let cmd = UpdateBannerCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id().clone(), f.region(), version_snapshot),
            new_banner_url: new_url.clone(),
        };

        // Act
        f.bus()
            .execute::<ProfileContext, UpdateBannerCommand, ()>(f.profile_ctx().clone(), cmd)
            .await?;

        // Assert
        f.assert_profile(|p| {
            assert_eq!(p.banner(), Some(&new_url));
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

        f.assert_outbox(1, Some(ProfileEvent::BANNER_UPDATED));

        Ok(())
    }

    #[tokio::test]
    async fn test_update_banner_technical_idempotency() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let cmd_id = Uuid::new_v4();
        f.idempotency_repo().seed(cmd_id);

        let profile = f.builder("alice").build()?;
        f.given_profile(profile).await;

        let cmd = UpdateBannerCommand {
            command_id: cmd_id,
            target: CommandTarget::new(f.profile_id().clone(), f.region(), 0),
            new_banner_url: Url::try_new("https://cdn.test.com/any.png")?,
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, UpdateBannerCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        match result {
            Err(e) => {
                assert_eq!(e.code, ErrorCode::AlreadyExists);
                assert!(e.message.contains("Command"));
            }
            Ok(_) => panic!("Should have failed with AlreadyExists"),
        }
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_banner_business_idempotency() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let current_url = Url::try_new("https://cdn.test.com/banner.png")?;

        let profile = f
            .builder("alice")
            .with_banner(current_url.clone())
            .build()?;
        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let cmd = UpdateBannerCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id().clone(), f.region(), version_snapshot),
            new_banner_url: current_url, // Même URL
        };

        // Act
        f.bus()
            .execute::<ProfileContext, UpdateBannerCommand, ()>(f.profile_ctx().clone(), cmd)
            .await?;

        // Assert
        f.assert_profile(|p| {
            assert_eq!(p.version(), version_snapshot); // Pas de save
        })
        .await;
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_banner_concurrency_conflict() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let profile = f.builder("alice").build()?;
        f.given_profile(profile).await;

        let cmd = UpdateBannerCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id().clone(), f.region(), 10), // Version erronée
            new_banner_url: Url::try_new("https://cdn.test.com/fail.png")?,
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, UpdateBannerCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(e) if e.code == ErrorCode::ConcurrencyConflict
        ));

        Ok(())
    }
}

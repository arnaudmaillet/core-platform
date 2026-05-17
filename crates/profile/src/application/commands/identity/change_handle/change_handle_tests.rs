// crates/profile/src/application/commands/identity/change_handle/change_handle_handler.rs

#[cfg(test)]
mod tests {
    use crate::application::utils::ProfileTestFixture;
    use crate::commands::ChangeHandleCommand;
    use crate::context::ProfileContext;
    use crate::entities::Profile;
    use crate::events::ProfileEvent;
    use crate::types::Handle;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{ErrorCode, Result, Versioned};
    use shared_kernel::types::{AccountId, ProfileId};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_change_handle_success() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let profile = f.builder("old.handle").build()?;
        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let new_handle = Handle::try_new("new.handle")?;

        let cmd = ChangeHandleCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id().clone(), f.region(), version_snapshot),
            new_handle: new_handle.clone(),
        };

        // Act
        f.bus()
            .execute::<ProfileContext, ChangeHandleCommand, ()>(f.profile_ctx().clone(), cmd)
            .await?;

        // Assert
        f.assert_profile(|p| {
            assert_eq!(p.handle(), &new_handle);
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

        f.assert_outbox(1, Some(ProfileEvent::HANDLE_CHANGED));

        Ok(())
    }

    #[tokio::test]
    async fn test_change_handle_conflict_already_exists() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();

        // 1. On crée le profil cible (celui qu'on veut modifier)
        let profile = f.builder("my.handle").build()?;
        f.given_profile(profile).await;

        // 2. On crée un AUTRE profil qui possède déjà le handle "taken.handle"
        let other_id = ProfileId::generate();
        let taken_handle = Handle::try_new("taken.handle")?;
        let other_profile =
            Profile::builder(AccountId::generate(f.region()), taken_handle.clone())?
                .with_profile_id(other_id)
                .build()?;

        f.given_profile(other_profile).await;

        // 3. On essaie de donner le handle déjà pris à notre profil initial
        let cmd = ChangeHandleCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id().clone(), f.region(), 0),
            new_handle: taken_handle,
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, ChangeHandleCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        match result {
            Err(e) => {
                assert_eq!(e.code, ErrorCode::AlreadyExists);
                assert!(e.message.contains("Profile"));
                assert!(e.message.contains("handle"));
            }
            Ok(_) => panic!("Should have failed with AlreadyExists"),
        }

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_change_handle_business_idempotency() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let handle = Handle::try_new("alice.handle")?;

        let profile = f.builder("alice.handle").build()?;
        let version_snapshot = profile.version();
        f.given_profile(profile).await;

        let cmd = ChangeHandleCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id().clone(), f.region(), version_snapshot),
            new_handle: handle,
        };

        // Act
        f.bus()
            .execute::<ProfileContext, ChangeHandleCommand, ()>(f.profile_ctx().clone(), cmd)
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
    async fn test_change_handle_concurrency_conflict() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let profile = f.builder("alice").build()?;
        f.given_profile(profile).await;

        let cmd = ChangeHandleCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.profile_id().clone(), f.region(), 99), // Mauvaise version
            new_handle: Handle::try_new("new.alice")?,
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, ChangeHandleCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(e) if e.code == ErrorCode::ConcurrencyConflict
        ));

        Ok(())
    }
}

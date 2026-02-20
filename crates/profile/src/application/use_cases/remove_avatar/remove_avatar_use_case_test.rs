// crates/profile/src/application/remove_avatar/remove_avatar_use_case_test.rs

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;
    use shared_kernel::domain::events::{EventEnvelope, DomainEvent, AggregateRoot};
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url}; // Suppression Username
    use shared_kernel::errors::{DomainError, Result};

    use crate::application::use_cases::remove_avatar::{RemoveAvatarCommand, RemoveAvatarUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{DisplayName, Handle, ProfileId}; // Ajout Handle et ProfileId
    use crate::domain::repositories::ProfileRepositoryStub;

    fn setup(profile: Option<Profile>) -> RemoveAvatarUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        RemoveAvatarUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        )
    }

    #[tokio::test]
    async fn test_remove_avatar_success() {
        // Arrange
        let owner_id = AccountId::new(); // Renommé pour la clarté
        let region = RegionCode::try_new("eu").unwrap();
        let url = Url::try_new("https://cdn.com/old_photo.png").unwrap();

        let mut profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(), // Utilisation de Handle
        ).build();

        let profile_id = profile.id().clone();
        profile.update_avatar(&region, url).unwrap();

        let use_case = setup(Some(profile));
        let cmd = RemoveAvatarCommand {
            profile_id: profile_id.clone(),
            region: region.clone()
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated_profile = result.unwrap();
        assert!(updated_profile.avatar_url().is_none());
        assert_eq!(updated_profile.version(), 3);
    }

    #[tokio::test]
    async fn test_remove_avatar_fails_on_region_mismatch() {
        // Arrange
        let owner_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();
        let wrong_region = RegionCode::try_new("us").unwrap();

        let profile = Profile::builder(
            owner_id,
            actual_region,
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        ).build();

        let profile_id = profile.id().clone();
        let use_case = setup(Some(profile));

        let cmd = RemoveAvatarCommand {
            profile_id,
            region: wrong_region
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_remove_avatar_already_none() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        ).build();

        let profile_id = profile.id().clone();
        let use_case = setup(Some(profile));
        let cmd = RemoveAvatarCommand { profile_id, region };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated_profile = result.unwrap();
        assert!(updated_profile.avatar_url().is_none());
        assert_eq!(updated_profile.version(), 1);
    }

    #[tokio::test]
    async fn test_remove_avatar_profile_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = RemoveAvatarCommand {
            profile_id: ProfileId::new(),
            region: RegionCode::try_new("eu").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_remove_avatar_concurrency_conflict() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        ).build();

        let profile_id = profile.id().clone();
        profile.update_avatar(&region, Url::try_new("https://old.png").unwrap()).unwrap();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch in DB".into(),
            })),
            ..Default::default()
        });

        let use_case = RemoveAvatarUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        );

        // Act
        let result = use_case.execute(RemoveAvatarCommand { profile_id, region }).await;

        // Assert
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_remove_avatar_db_internal_error() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        let mut profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap()
        ).build();

        let profile_id = profile.id().clone();
        profile.update_avatar(&region, Url::try_new("https://photo.png").unwrap()).unwrap();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::Internal("timeout".into()))),
            ..Default::default()
        });

        let use_case = RemoveAvatarUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager));

        let result = use_case.execute(RemoveAvatarCommand { profile_id, region }).await;

        match result {
            Err(DomainError::Internal(m)) => assert!(m.contains("timeout")),
            _ => panic!("Expected Internal error, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_remove_avatar_outbox_error_rollbacks() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        ).build();

        let profile_id = profile.id().clone();
        profile.update_avatar(&region, Url::try_new("https://old.png").unwrap()).unwrap();

        struct FailingOutbox;
        #[async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(&self, _: &mut dyn shared_kernel::domain::transaction::Transaction, _: &dyn DomainEvent) -> Result<()> {
                Err(DomainError::Internal("Outbox Full".into()))
            }
            async fn find_pending(&self, _: i32) -> Result<Vec<EventEnvelope>> { Ok(vec![]) }
        }

        let use_case = RemoveAvatarUseCase::new(
            Arc::new(ProfileRepositoryStub {
                profile_to_return: Mutex::new(Some(profile)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        // Act
        let result = use_case.execute(RemoveAvatarCommand { profile_id, region }).await;

        // Assert
        assert!(result.is_err());
    }
}
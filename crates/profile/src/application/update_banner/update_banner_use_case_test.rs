// crates/profile/src/application/update_banner/update_banner_use_case_test.rs

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope, DomainEvent};
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url};
    use shared_kernel::errors::{DomainError, Result};

    use crate::application::update_banner::{UpdateBannerCommand, UpdateBannerUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{DisplayName, Handle, ProfileId}; // Ajout Handle et ProfileId
    use crate::domain::repositories::ProfileRepositoryStub;

    /// Helper pour instancier le Use Case avec les stubs
    fn setup(profile: Option<Profile>) -> UpdateBannerUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        UpdateBannerUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        )
    }

    #[tokio::test]
    async fn test_update_banner_success() {
        // Arrange
        let owner_id = AccountId::new(); // Contexte propriétaire
        let region = RegionCode::try_new("eu").unwrap();
        let initial_profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(), // Username -> Handle
        )
            .build();

        let profile_id = initial_profile.id().clone(); // Pivot identité
        let use_case = setup(Some(initial_profile));
        let new_url = Url::try_new("https://cdn.com/banner_v1.png").unwrap();

        let cmd = UpdateBannerCommand {
            profile_id: profile_id.clone(),
            region: region.clone(),
            new_banner_url: new_url.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.banner_url(), Some(&new_url));
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_banner_fails_on_region_mismatch() {
        // Arrange
        let owner_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();
        let wrong_region = RegionCode::try_new("us").unwrap();

        let profile = Profile::builder(
            owner_id,
            actual_region,
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        ).build();

        let profile_id = profile.id().clone();
        let use_case = setup(Some(profile));
        let new_url = Url::try_new("https://cdn.com/banner.png").unwrap();

        let cmd = UpdateBannerCommand {
            profile_id,
            region: wrong_region,
            new_banner_url: new_url,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_update_banner_idempotency() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let banner_url = Url::try_new("https://cdn.com/banner_v1.png").unwrap();

        let mut profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        )
            .build();

        let profile_id = profile.id().clone();
        profile.update_banner(&region, banner_url.clone()).unwrap();

        let use_case = setup(Some(profile));

        let cmd = UpdateBannerCommand {
            profile_id,
            region,
            new_banner_url: banner_url,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let profile = result.unwrap();
        assert_eq!(profile.version(), 2);
    }

    #[tokio::test]
    async fn test_update_banner_not_found() {
        // Arrange
        let use_case = setup(None);
        let url = Url::try_new("https://cdn.com/banner.png").unwrap();

        let cmd = UpdateBannerCommand {
            profile_id: ProfileId::new(),
            region: RegionCode::try_new("eu").unwrap(),
            new_banner_url: url,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_banner_change_existing() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        )
            .build();

        let profile_id = profile.id().clone();
        profile.update_banner(&region, Url::try_new("https://old.png").unwrap()).unwrap();

        let use_case = setup(Some(profile));
        let new_url = Url::try_new("https://new.png").unwrap();

        let cmd = UpdateBannerCommand {
            profile_id,
            region,
            new_banner_url: new_url.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.banner_url(), Some(&new_url));
        assert_eq!(updated.version(), 3);
    }

    #[tokio::test]
    async fn test_update_banner_concurrency_conflict() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        )
            .build();

        let profile_id = profile.id().clone();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Conflict during save".into(),
            })),
            ..Default::default()
        });

        let use_case = UpdateBannerUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        );

        let cmd = UpdateBannerCommand {
            profile_id,
            region,
            new_banner_url: Url::try_new("https://new.png").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_update_banner_repository_error() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        ).build();

        let profile_id = profile.id().clone();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::Internal("SQL Error".into()))),
            ..Default::default()
        });

        let use_case = UpdateBannerUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        );

        // Act
        let result = use_case.execute(UpdateBannerCommand {
            profile_id,
            region,
            new_banner_url: Url::try_new("https://new.png").unwrap(),
        }).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Internal(m)) if m == "SQL Error"));
    }

    #[tokio::test]
    async fn test_update_banner_outbox_failure_rollbacks() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        ).build();

        let profile_id = profile.id().clone();

        struct FailingOutbox;
        #[async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(&self, _: &mut dyn shared_kernel::domain::transaction::Transaction, _: &dyn DomainEvent) -> Result<()> {
                Err(DomainError::Internal("Outbox capacity reached".into()))
            }
            async fn find_pending(&self, _: i32) -> Result<Vec<EventEnvelope>> { Ok(vec![]) }
        }

        let use_case = UpdateBannerUseCase::new(
            Arc::new(ProfileRepositoryStub {
                profile_to_return: Mutex::new(Some(profile)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        // Act
        let result = use_case.execute(UpdateBannerCommand {
            profile_id,
            region,
            new_banner_url: Url::try_new("https://new.png").unwrap(),
        }).await;

        // Assert
        assert!(result.is_err());
    }
}
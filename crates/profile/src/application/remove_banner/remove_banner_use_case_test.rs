// crates/profile/src/application/remove_banner/remove_banner_use_case_test.rs

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope, DomainEvent};
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url};
    use shared_kernel::errors::{DomainError, Result};

    use crate::application::remove_banner::{RemoveBannerCommand, RemoveBannerUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{DisplayName, Handle, ProfileId};
    use crate::domain::repositories::ProfileRepositoryStub;

    /// Helper pour configurer le Use Case avec un état initial
    fn setup(profile: Option<Profile>) -> RemoveBannerUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        RemoveBannerUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        )
    }

    #[tokio::test]
    async fn test_remove_banner_success() {
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

        // On ajoute une bannière
        let banner_url = Url::try_new("https://cdn.com/banner.png").unwrap();
        profile.update_banner(&region, banner_url).unwrap();

        let use_case = setup(Some(profile));
        let cmd = RemoveBannerCommand {
            profile_id: profile_id.clone(),
            region: region.clone()
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated_profile = result.unwrap();

        assert!(updated_profile.banner_url().is_none());
        assert_eq!(updated_profile.version(), 3);
    }

    #[tokio::test]
    async fn test_remove_banner_fails_on_region_mismatch() {
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

        let cmd = RemoveBannerCommand {
            profile_id,
            region: wrong_region
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_remove_banner_already_empty() {
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
        let cmd = RemoveBannerCommand { profile_id, region };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let returned_profile = result.unwrap();
        assert!(returned_profile.banner_url().is_none());
        assert_eq!(returned_profile.version(), 1);
    }

    #[tokio::test]
    async fn test_remove_banner_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = RemoveBannerCommand {
            profile_id: ProfileId::new(),
            region: RegionCode::try_new("eu").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_remove_banner_concurrency_conflict() {
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
        profile.update_banner(&region, Url::try_new("https://old.png").unwrap()).unwrap();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version changed by another process".into(),
            })),
            ..Default::default()
        });

        let use_case = RemoveBannerUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        );

        let cmd = RemoveBannerCommand { profile_id, region };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_remove_banner_repository_internal_error() {
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        let mut profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap()
        ).build();

        let profile_id = profile.id().clone();
        profile.update_banner(&region, Url::try_new("https://banner.png").unwrap()).unwrap();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::Internal("Database is down".into()))),
            ..Default::default()
        });

        let use_case = RemoveBannerUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager));

        let result = use_case.execute(RemoveBannerCommand { profile_id, region }).await;

        match result {
            Err(DomainError::Internal(m)) => assert_eq!(m, "Database is down"),
            _ => panic!("Expected Internal error, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_remove_banner_outbox_failure_rollbacks() {
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
        profile.update_banner(&region, Url::try_new("https://old.png").unwrap()).unwrap();

        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(&self, _: &mut dyn shared_kernel::domain::transaction::Transaction, _: &dyn DomainEvent) -> Result<()> {
                Err(DomainError::Internal("Outbox disk full".into()))
            }
            async fn find_pending(&self, _: i32) -> Result<Vec<EventEnvelope>> { Ok(vec![]) }
        }

        let use_case = RemoveBannerUseCase::new(
            Arc::new(ProfileRepositoryStub {
                profile_to_return: Mutex::new(Some(profile)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        // Act
        let result = use_case.execute(RemoveBannerCommand { profile_id, region }).await;

        // Assert
        assert!(result.is_err());
    }
}
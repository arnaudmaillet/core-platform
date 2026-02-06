// crates/profile/src/application/remove_banner/remove_banner_use_case_test.rs

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope, DomainEvent};
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url, Username};
    use shared_kernel::errors::{DomainError, Result};

    use crate::application::remove_banner::{RemoveBannerCommand, RemoveBannerUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;
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
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
            .build();

        // On ajoute une bannière (nécessite la région pour le check de sharding)
        let banner_url = Url::try_new("https://cdn.com/banner.png").unwrap();
        profile.update_banner(&region, banner_url).unwrap();

        let use_case = setup(Some(profile));
        let cmd = RemoveBannerCommand {
            account_id: account_id.clone(),
            region: region.clone()
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated_profile = result.unwrap();

        assert!(
            updated_profile.banner_url().is_none(),
            "La bannière devrait être supprimée"
        );
        // version: 1 (init) + 1 (update) + 1 (remove) = 3
        assert_eq!(updated_profile.version(), 3);
    }

    #[tokio::test]
    async fn test_remove_banner_fails_on_region_mismatch() {
        // Arrange : Profil en EU, Commande en US
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();
        let wrong_region = RegionCode::try_new("us").unwrap();

        let profile = Profile::builder(
            account_id.clone(),
            actual_region,
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        ).build();

        let use_case = setup(Some(profile));

        let cmd = RemoveBannerCommand {
            account_id,
            region: wrong_region
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_remove_banner_already_empty() {
        // Arrange : Profil sans bannière
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
            .build();

        let use_case = setup(Some(profile));
        let cmd = RemoveBannerCommand { account_id, region };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let returned_profile = result.unwrap();
        assert!(returned_profile.banner_url().is_none());
        // Idempotence : la version n'a pas bougé
        assert_eq!(returned_profile.version(), 1);
    }

    #[tokio::test]
    async fn test_remove_banner_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = RemoveBannerCommand {
            account_id: AccountId::new(),
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
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
            .build();
        profile.update_banner(&region, Url::try_new("https://old.png").unwrap()).unwrap();

        // Conflit simulé au save
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

        let cmd = RemoveBannerCommand { account_id, region };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_remove_banner_repository_internal_error() {
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // IMPORTANT : On ajoute une bannière pour forcer le passage à la sauvegarde
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap()
        ).build();

        profile.update_banner(&region, Url::try_new("https://banner.png").unwrap()).unwrap();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::Internal("Database is down".into()))),
            ..Default::default()
        });

        let use_case = RemoveBannerUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager));

        let result = use_case.execute(RemoveBannerCommand { account_id, region }).await;

        match result {
            Err(DomainError::Internal(m)) => assert_eq!(m, "Database is down"),
            _ => panic!("Expected Internal error, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_remove_banner_outbox_failure_rollbacks() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
            .build();
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
        let result = use_case.execute(RemoveBannerCommand { account_id, region }).await;

        // Assert
        assert!(result.is_err());
    }
}
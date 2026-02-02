#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use shared_kernel::domain::events::AggregateRoot;
    use crate::utils::profile_repository_stub::{ProfileRepositoryStub, OutboxRepoStub, StubTxManager};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;
    use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode, Url};
    use shared_kernel::errors::DomainError;
    use crate::application::update_banner::{UpdateBannerCommand, UpdateBannerUseCase};
    use crate::domain::builders::ProfileBuilder;

    /// Helper pour instancier le Use Case avec les stubs
    fn setup(profile: Option<Profile>) -> UpdateBannerUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            exists_return: Mutex::new(false),
            error_to_return: Mutex::new(None),
        });

        UpdateBannerUseCase::new(
            repo,
            Arc::new(OutboxRepoStub),
            Arc::new(StubTxManager),
        )
    }

    #[tokio::test]
    async fn test_update_banner_success() {
        // Arrange : Profil initial sans bannière
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let initial_profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap()
        ).build();

        let use_case = setup(Some(initial_profile));
        let new_url = Url::try_from("https://cdn.com/banner_v1.png".to_string()).unwrap();

        let cmd = UpdateBannerCommand {
            account_id,
            region,
            new_banner_url: new_url.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        // On vérifie que la bannière est bien présente via le getter
        assert_eq!(updated.banner_url(), Some(&new_url));
        // Version : 1 (création) + 1 (update_banner) = 2
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_banner_idempotency() {
        // Arrange : Profil qui a DÉJÀ cette bannière
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let banner_url = Url::try_from("https://cdn.com/banner_v1.png".to_string()).unwrap();

        let mut profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap()
        ).build();
        profile.update_banner(banner_url.clone()); // Version passe à 2

        let use_case = setup(Some(profile));

        let cmd = UpdateBannerCommand {
            account_id,
            region,
            new_banner_url: banner_url, // On renvoie la même URL
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let profile = result.unwrap();
        // L'idempotence métier doit empêcher une nouvelle version (reste à 2)
        assert_eq!(profile.version(), 2);
    }

    #[tokio::test]
    async fn test_update_banner_not_found() {
        // Arrange : Pas de profil en DB
        let use_case = setup(None);
        let url = Url::try_from("https://cdn.com/banner.png".to_string()).unwrap();

        let cmd = UpdateBannerCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
            new_banner_url: url,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_banner_change_existing() {
        // Arrange : On passe d'une bannière A à une bannière B
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap()
        ).build();
        profile.update_banner(Url::try_from("https://old.png".to_string()).unwrap());

        let use_case = setup(Some(profile));
        let new_url = Url::try_from("https://new.png".to_string()).unwrap();

        let cmd = UpdateBannerCommand {
            account_id,
            region,
            new_banner_url: new_url.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.banner_url(), Some(&new_url));
        assert_eq!(updated.version(), 3); // Create(1) + Old(2) + New(3)
    }

    #[tokio::test]
    async fn test_update_banner_concurrency_conflict() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap()
        ).build();

        // Configuration du Stub pour simuler une collision de version au moment du save
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Autre mise à jour simultanée détectée".into()
            })),
            ..Default::default()
        });

        let use_case = UpdateBannerUseCase::new(
            repo,
            Arc::new(OutboxRepoStub),
            Arc::new(StubTxManager),
        );

        let cmd = UpdateBannerCommand {
            account_id,
            region,
            new_banner_url: Url::try_from("https://new.png".to_string()).unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        // On vérifie que l'erreur de concurrence remonte bien
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_update_banner_repository_error() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Alice"), Username::try_new("alice").unwrap()).build();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::Internal("Erreur disque SQL".into()))),
            ..Default::default()
        });

        let use_case = UpdateBannerUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        // Act
        let result = use_case.execute(UpdateBannerCommand {
            account_id,
            region,
            new_banner_url: Url::try_from("https://new.png".to_string()).unwrap(),
        }).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Internal(m)) if m == "Erreur disque SQL"));
    }

    #[tokio::test]
    async fn test_update_banner_outbox_failure_rollbacks() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Alice"), Username::try_new("alice").unwrap()).build();

        // Stub Outbox qui crash systématiquement
        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(&self, _: &mut dyn shared_kernel::domain::transaction::Transaction, _: &dyn shared_kernel::domain::events::DomainEvent) -> shared_kernel::errors::Result<()> {
                Err(DomainError::Internal("Outbox capacity reached".into()))
            }
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
            account_id,
            region,
            new_banner_url: Url::try_from("https://new.png".to_string()).unwrap(),
        }).await;

        // Assert
        // Le Use Case doit échouer si l'Outbox échoue (garantie transactionnelle)
        assert!(result.is_err());
    }
}
#[cfg(test)]
mod tests {
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope};
    use std::sync::{Arc, Mutex};
    // On réutilise nos outils de test centralisés
    use crate::application::remove_banner::{RemoveBannerCommand, RemoveBannerUseCase};
    use crate::domain::builders::ProfileBuilder;
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;
    use crate::utils::profile_repository_stub::{
        OutboxRepoStub, ProfileRepositoryStub, StubTxManager,
    };
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url, Username};
    use shared_kernel::errors::DomainError;

    /// Helper pour configurer le Use Case avec un état initial
    fn setup(profile: Option<Profile>) -> RemoveBannerUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            exists_return: Mutex::new(false),
            error_to_return: Mutex::new(None),
        });

        RemoveBannerUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager))
    }

    #[tokio::test]
    async fn test_remove_banner_success() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();

        // On ajoute une bannière via le mutateur métier
        let banner_url = Url::try_from("https://cdn.com/banner.png".to_string()).unwrap();
        profile.update_banner(banner_url);

        let use_case = setup(Some(profile));
        let cmd = RemoveBannerCommand { account_id, region };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated_profile = result.unwrap();

        // Vérification via le getter
        assert!(
            updated_profile.banner_url().is_none(),
            "La bannière devrait être supprimée"
        );
        assert_eq!(updated_profile.version(), 3); // 1 (création) + 1 (update_banner) + 1 (remove_banner)
    }

    #[tokio::test]
    async fn test_remove_banner_already_empty() {
        // Arrange : Profil sans bannière
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(
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
        // La version ne devrait pas avoir bougé car remove_banner() a retourné false (idempotence)
        assert_eq!(returned_profile.version(), 1);
    }

    #[tokio::test]
    async fn test_remove_banner_not_found() {
        // Arrange : Aucun profil trouvé dans le repo
        let use_case = setup(None);

        let cmd = RemoveBannerCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_remove_banner_concurrency_conflict() {
        // Arrange : On simule un profil existant
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();
        profile.update_banner(Url::try_from("https://old.png".to_string()).unwrap());

        // On configure le stub pour renvoyer un conflit au moment du save (étape 4 du Use Case)
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version changed by another process".into(),
            })),
            ..Default::default()
        });

        let use_case =
            RemoveBannerUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        let cmd = RemoveBannerCommand { account_id, region };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_remove_banner_repository_internal_error() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();
        profile.update_banner(Url::try_from("https://old.png".to_string()).unwrap());

        // On simule une erreur SQL critique
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::Internal("Database is down".into()))),
            ..Default::default()
        });

        let use_case =
            RemoveBannerUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        // Act
        let result = use_case
            .execute(RemoveBannerCommand { account_id, region })
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::Internal(m)) if m == "Database is down"));
    }

    #[tokio::test]
    async fn test_remove_banner_outbox_failure_rollbacks() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();
        profile.update_banner(Url::try_from("https://old.png".to_string()).unwrap());

        // On crée un Stub Outbox qui crash
        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(
                &self,
                _: &mut dyn shared_kernel::domain::transaction::Transaction,
                _: &dyn shared_kernel::domain::events::DomainEvent,
            ) -> shared_kernel::errors::Result<()> {
                Err(DomainError::Internal("Outbox disk full".into()))
            }

            async fn find_pending(&self, _limit: i32) -> shared_kernel::errors::Result<Vec<EventEnvelope>> {
                Ok(vec![])
            }
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
        let result = use_case
            .execute(RemoveBannerCommand { account_id, region })
            .await;

        // Assert
        // Si l'outbox échoue, le Use Case doit remonter l'erreur et la transaction échoue
        assert!(result.is_err());
    }
}

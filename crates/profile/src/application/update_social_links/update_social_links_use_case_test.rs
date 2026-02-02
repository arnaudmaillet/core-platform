#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode, Url};
    use shared_kernel::errors::DomainError;
    use crate::application::update_social_links::{UpdateSocialLinksCommand, UpdateSocialLinksUseCase};
    use crate::domain::builders::ProfileBuilder;
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{DisplayName, SocialLinks};
    use crate::utils::profile_repository_stub::{ProfileRepositoryStub, OutboxRepoStub, StubTxManager};

    /// Helper pour configurer le Use Case avec un état initial
    fn setup(profile: Option<Profile>) -> UpdateSocialLinksUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        UpdateSocialLinksUseCase::new(
            repo,
            Arc::new(OutboxRepoStub),
            Arc::new(StubTxManager),
        )
    }

    #[tokio::test]
    async fn test_update_social_links_success() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap()
        ).build();

        let use_case = setup(Some(profile));

        // Construction des SocialLinks avec le type Url
        let mut links = SocialLinks::default();
        links.website = Some(Url::try_from("https://alice.dev".to_string()).unwrap());
        links.x = Some(Url::try_from("https://x.com/alice".to_string()).unwrap());
        links.linkedin = Some(Url::try_from("https://linkedin.com/in/alice".to_string()).unwrap());

        let cmd = UpdateSocialLinksCommand {
            account_id,
            region,
            new_links: Some(links.clone()),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.social_links(), Some(&links));
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_clear_social_links_success() {
        // Arrange : Profil avec des liens existants
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap()
        ).build();

        let mut initial_links = SocialLinks::default();
        initial_links.website = Some(Url::try_from("https://old.com".to_string()).unwrap());
        profile.update_social_links(Some(initial_links)); // v2

        let use_case = setup(Some(profile));

        let cmd = UpdateSocialLinksCommand {
            account_id,
            region,
            new_links: None,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert!(updated.social_links().is_none());
        assert_eq!(updated.version(), 3);
    }

    #[tokio::test]
    async fn test_update_social_links_idempotency() {
        // Arrange : On renvoie exactement les mêmes liens
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut links = SocialLinks::default();
        links.website = Some(Url::try_from("https://same.com".to_string()).unwrap());

        let mut profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Alice"), Username::try_new("alice").unwrap()).build();
        profile.update_social_links(Some(links.clone())); // v2

        let use_case = setup(Some(profile));

        let cmd = UpdateSocialLinksCommand {
            account_id,
            region,
            new_links: Some(links),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        // L'idempotence doit empêcher une nouvelle transaction et un nouvel incrément (reste à 2)
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_social_links_not_found() {
        // Arrange : Pas de profil en DB
        let use_case = setup(None);
        let cmd = UpdateSocialLinksCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
            new_links: None,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_social_links_concurrency_conflict() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Alice"), Username::try_new("alice").unwrap()).build();

        // Simulation d'un échec d'Optimistic Locking
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch detected by repo".into()
            })),
            ..Default::default()
        });

        let use_case = UpdateSocialLinksUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        let cmd = UpdateSocialLinksCommand {
            account_id,
            region,
            new_links: Some(SocialLinks::default()),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_update_social_links_atomic_rollback_on_outbox_failure() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Alice"), Username::try_new("alice").unwrap()).build();

        // On crée un stub Outbox qui échoue systématiquement
        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(&self, _: &mut dyn shared_kernel::domain::transaction::Transaction, _: &dyn shared_kernel::domain::events::DomainEvent) -> shared_kernel::errors::Result<()> {
                Err(DomainError::Internal("Outbox error".into()))
            }
        }

        let use_case = UpdateSocialLinksUseCase::new(
            Arc::new(ProfileRepositoryStub {
                profile_to_return: Mutex::new(Some(profile)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        let mut links = SocialLinks::default();
        links.website = Some(Url::try_from("https://fail.com".to_string()).unwrap());

        // Act
        let result = use_case.execute(UpdateSocialLinksCommand {
            account_id,
            region,
            new_links: Some(links),
        }).await;

        // Assert
        // Si l'Outbox échoue, le Use Case doit remonter l'erreur pour forcer le Rollback
        assert!(result.is_err());
    }
}
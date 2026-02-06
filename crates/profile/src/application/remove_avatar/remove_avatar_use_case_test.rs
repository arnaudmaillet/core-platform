// crates/profile/src/application/remove_avatar/remove_avatar_use_case_test.rs

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;
    use shared_kernel::domain::events::{EventEnvelope, DomainEvent, AggregateRoot};
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url, Username};
    use shared_kernel::errors::{DomainError, Result};

    use crate::application::remove_avatar::{RemoveAvatarCommand, RemoveAvatarUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;
    use crate::domain::repositories::ProfileRepositoryStub;

    /// Helper pour instancier le Use Case avec ses dépendances
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
        // Arrange : Un profil qui possède un avatar
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let url = Url::try_new("https://cdn.com/old_photo.png").unwrap();

        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
            .build();

        // On simule un état existant avec avatar (version 2)
        profile.update_avatar(&region, url).unwrap();

        let use_case = setup(Some(profile));
        let cmd = RemoveAvatarCommand {
            account_id: account_id.clone(),
            region: region.clone()
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated_profile = result.unwrap();
        assert!(updated_profile.avatar_url().is_none());
        assert_eq!(updated_profile.version(), 3); // Initial(1) -> Set(2) -> Remove(3)
    }

    #[tokio::test]
    async fn test_remove_avatar_fails_on_region_mismatch() {
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

        let cmd = RemoveAvatarCommand {
            account_id,
            region: wrong_region
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        // Doit renvoyer Forbidden car le check est dans l'entité
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_remove_avatar_already_none() {
        // Arrange : Un profil qui n'a déjà pas d'avatar
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        ).build();

        let use_case = setup(Some(profile));
        let cmd = RemoveAvatarCommand { account_id, region };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated_profile = result.unwrap();
        assert!(updated_profile.avatar_url().is_none());
        // L'idempotence a fonctionné : la version n'a pas bougé
        assert_eq!(updated_profile.version(), 1);
    }

    #[tokio::test]
    async fn test_remove_avatar_profile_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = RemoveAvatarCommand {
            account_id: AccountId::new(),
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
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        ).build();

        profile.update_avatar(&region, Url::try_new("https://old.png").unwrap()).unwrap();

        // Stub configuré pour renvoyer une erreur de version au save
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
        let result = use_case.execute(RemoveAvatarCommand { account_id, region }).await;

        // Assert
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_remove_avatar_db_internal_error() {
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // IMPORTANT : On crée un profil qui A un avatar pour forcer la mutation
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap()
        ).build();

        // On ajoute un avatar (version passe à 2)
        profile.update_avatar(&region, Url::try_new("https://photo.png").unwrap()).unwrap();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::Internal("timeout".into()))),
            ..Default::default()
        });

        let use_case = RemoveAvatarUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager));

        let result = use_case.execute(RemoveAvatarCommand { account_id, region }).await;

        match result {
            Err(DomainError::Internal(m)) => assert!(m.contains("timeout")),
            _ => panic!("Expected Internal error, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_remove_avatar_outbox_error_rollbacks() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        ).build();
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
        let result = use_case.execute(RemoveAvatarCommand { account_id, region }).await;

        // Assert
        // L'échec de l'outbox doit faire échouer le use case
        assert!(result.is_err());
    }
}
// crates/profile/src/application/update_bio/update_bio_use_case_test.rs

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope, DomainEvent};
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
    use shared_kernel::errors::{DomainError, Result};

    use crate::application::update_bio::{UpdateBioCommand, UpdateBioUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{Bio, DisplayName};
    use crate::domain::repositories::ProfileRepositoryStub;

    /// Helper pour configurer le Use Case avec ses dépendances
    fn setup(profile: Option<Profile>) -> UpdateBioUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        UpdateBioUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        )
    }

    #[tokio::test]
    async fn test_update_bio_success() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let initial_profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
            .build();

        let use_case = setup(Some(initial_profile));
        let new_bio = Some(Bio::try_new("Hello World").unwrap());

        let cmd = UpdateBioCommand {
            account_id: account_id.clone(),
            region: region.clone(),
            new_bio: new_bio.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.bio(), new_bio.as_ref());
        assert_eq!(updated.version(), 2); // Init(1) + Update(2)
    }

    #[tokio::test]
    async fn test_update_bio_fails_on_region_mismatch() {
        // Arrange : Profil en EU, Commande en US
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();
        let wrong_region = RegionCode::try_new("us").unwrap();

        let profile = Profile::builder(
            account_id.clone(),
            actual_region,
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        ).build();

        let use_case = setup(Some(profile));
        let new_bio = Some(Bio::try_new("Illegal Update").unwrap());

        let cmd = UpdateBioCommand {
            account_id,
            region: wrong_region,
            new_bio,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert : Doit être bloqué par l'entité
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_remove_bio_success() {
        // Arrange : Profil ayant déjà une bio
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
            .build();

        // Setup initial avec bio (version 2)
        profile.update_bio(&region, Some(Bio::try_new("Old Bio").unwrap())).unwrap();

        let use_case = setup(Some(profile));

        let cmd = UpdateBioCommand {
            account_id,
            region,
            new_bio: None, // Suppression
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert!(updated.bio().is_none());
        assert_eq!(updated.version(), 3); // Init(1) + Old(2) + Remove(3)
    }

    #[tokio::test]
    async fn test_update_bio_idempotency() {
        // Arrange : Nouvelle bio identique à l'ancienne
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let bio_text = Some(Bio::try_new("Consistent Bio").unwrap());

        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
            .build();

        // On fixe déjà la bio (version 2)
        profile.update_bio(&region, bio_text.clone()).unwrap();

        let use_case = setup(Some(profile));

        let cmd = UpdateBioCommand {
            account_id,
            region,
            new_bio: bio_text,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        // L'idempotence bloque l'incrément de version (reste 2)
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_bio_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = UpdateBioCommand {
            account_id: AccountId::new(),
            region: RegionCode::try_new("eu").unwrap(),
            new_bio: None,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_bio_concurrency_conflict() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
            .build();

        // On simule une collision de version lors de la sauvegarde
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch".into(),
            })),
            ..Default::default()
        });

        let use_case = UpdateBioUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        );

        // Act
        let result = use_case
            .execute(UpdateBioCommand {
                account_id,
                region,
                new_bio: Some(Bio::try_new("New Bio").unwrap()),
            })
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_update_bio_transaction_atomic_failure() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
            .build();

        struct FailingOutbox;
        #[async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(&self, _: &mut dyn shared_kernel::domain::transaction::Transaction, _: &dyn DomainEvent) -> Result<()> {
                Err(DomainError::Internal("Outbox capacity reached".into()))
            }
            async fn find_pending(&self, _limit: i32) -> Result<Vec<EventEnvelope>> { Ok(vec![]) }
        }

        let use_case = UpdateBioUseCase::new(
            Arc::new(ProfileRepositoryStub {
                profile_to_return: Mutex::new(Some(profile)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        // Act
        let result = use_case
            .execute(UpdateBioCommand {
                account_id,
                region,
                new_bio: Some(Bio::try_new("Failing Update").unwrap()),
            })
            .await;

        // Assert
        // Si l'Outbox échoue, le Use Case remonte l'erreur et rien n'est persisté
        assert!(result.is_err());
    }
}
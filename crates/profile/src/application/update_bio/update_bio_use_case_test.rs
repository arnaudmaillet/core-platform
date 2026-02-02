#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use shared_kernel::domain::events::AggregateRoot;
    use crate::utils::profile_repository_stub::{ProfileRepositoryStub, OutboxRepoStub, StubTxManager};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{Bio, DisplayName};
    use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode};
    use shared_kernel::errors::DomainError;
    use crate::application::update_bio::{UpdateBioCommand, UpdateBioUseCase};
    use crate::domain::builders::ProfileBuilder;

    /// Helper pour configurer le Use Case
    fn setup(profile: Option<Profile>) -> UpdateBioUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        UpdateBioUseCase::new(
            repo,
            Arc::new(OutboxRepoStub),
            Arc::new(StubTxManager),
        )
    }

    #[tokio::test]
    async fn test_update_bio_success() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let initial_profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap()
        ).build();

        let use_case = setup(Some(initial_profile));
        let new_bio = Some(Bio::try_new("Hello World").unwrap());

        let cmd = UpdateBioCommand {
            account_id,
            region,
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
    async fn test_remove_bio_success() {
        // Arrange : Profil ayant déjà une bio
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap()
        ).build();
        profile.update_bio(Some(Bio::try_new("Old Bio").unwrap()));

        let use_case = setup(Some(profile));

        let cmd = UpdateBioCommand {
            account_id,
            region,
            new_bio: None, // Suppression de la bio
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
        let region = RegionCode::from_raw("eu");
        let bio_text = Some(Bio::try_new("Consistent Bio").unwrap());

        let mut profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Alice"), Username::try_new("alice").unwrap()).build();
        profile.update_bio(bio_text.clone());

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
        // La version ne doit pas augmenter si la bio est identique
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_bio_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = UpdateBioCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
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
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Alice"), Username::try_new("alice").unwrap()).build();

        // Simulation d'un conflit de version (Optimistic Locking)
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch".into()
            })),
            ..Default::default()
        });

        let use_case = UpdateBioUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        // Act
        let result = use_case.execute(UpdateBioCommand {
            account_id,
            region,
            new_bio: Some(Bio::try_new("New Bio").unwrap()),
        }).await;

        // Assert
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_update_bio_transaction_atomic_failure() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Alice"), Username::try_new("alice").unwrap()).build();

        // Stub Outbox qui crash pour forcer un échec de transaction
        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(&self, _: &mut dyn shared_kernel::domain::transaction::Transaction, _: &dyn shared_kernel::domain::events::DomainEvent) -> shared_kernel::errors::Result<()> {
                Err(DomainError::Internal("Outbox error".into()))
            }
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
        let result = use_case.execute(UpdateBioCommand {
            account_id,
            region,
            new_bio: Some(Bio::try_new("Failing Update").unwrap()),
        }).await;

        // Assert
        assert!(result.is_err());
    }
}
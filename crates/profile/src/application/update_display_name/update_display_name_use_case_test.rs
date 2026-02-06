// crates/profile/src/application/update_display_name/update_display_name_use_case_test.rs

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope, DomainEvent};
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
    use shared_kernel::errors::{DomainError, Result};

    use crate::application::update_display_name::{UpdateDisplayNameCommand, UpdateDisplayNameUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;
    use crate::domain::repositories::ProfileRepositoryStub;

    /// Helper pour instancier le Use Case avec ses dépendances
    fn setup(profile: Option<Profile>) -> UpdateDisplayNameUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        UpdateDisplayNameUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        )
    }

    #[tokio::test]
    async fn test_update_display_name_success() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let initial_profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Old Name"),
            Username::try_new("user123").unwrap(),
        )
            .build();

        let use_case = setup(Some(initial_profile));
        let new_display_name = DisplayName::from_raw("New Name");

        let cmd = UpdateDisplayNameCommand {
            account_id: account_id.clone(),
            region: region.clone(),
            new_display_name: new_display_name.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.display_name(), &new_display_name);
        assert_eq!(updated.version(), 2); // Initial(1) -> Updated(2)
    }

    #[tokio::test]
    async fn test_update_display_name_fails_on_region_mismatch() {
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

        let cmd = UpdateDisplayNameCommand {
            account_id,
            region: wrong_region,
            new_display_name: DisplayName::from_raw("Hacker Name"),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert : Doit être bloqué par l'entité
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_update_display_name_idempotency() {
        // Arrange : Nom identique à l'existant
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let current_name = DisplayName::from_raw("Alice");

        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            current_name.clone(),
            Username::try_new("alice").unwrap(),
        ).build();

        let use_case = setup(Some(profile));

        let cmd = UpdateDisplayNameCommand {
            account_id,
            region,
            new_display_name: current_name,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        // La version ne doit pas avoir bougé (reste 1) car l'entité a retourné false
        assert_eq!(updated.version(), 1);
    }

    #[tokio::test]
    async fn test_update_display_name_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = UpdateDisplayNameCommand {
            account_id: AccountId::new(),
            region: RegionCode::try_new("eu").unwrap(),
            new_display_name: DisplayName::from_raw("Ghost"),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_display_name_concurrency_conflict() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        ).build();

        // Stub simulant une collision de version lors du save
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch".into(),
            })),
            ..Default::default()
        });

        let use_case = UpdateDisplayNameUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        );

        let cmd = UpdateDisplayNameCommand {
            account_id,
            region,
            new_display_name: DisplayName::from_raw("New Alice"),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_update_display_name_transaction_failure() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        ).build();

        struct FailingOutbox;
        #[async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(&self, _: &mut dyn shared_kernel::domain::transaction::Transaction, _: &dyn DomainEvent) -> Result<()> {
                Err(DomainError::Internal("Disk full".into()))
            }
            async fn find_pending(&self, _: i32) -> Result<Vec<EventEnvelope>> { Ok(vec![]) }
        }

        let use_case = UpdateDisplayNameUseCase::new(
            Arc::new(ProfileRepositoryStub {
                profile_to_return: Mutex::new(Some(profile)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        // Act
        let result = use_case.execute(UpdateDisplayNameCommand {
            account_id,
            region,
            new_display_name: DisplayName::from_raw("Failing Name"),
        }).await;

        // Assert
        assert!(result.is_err());
    }
}
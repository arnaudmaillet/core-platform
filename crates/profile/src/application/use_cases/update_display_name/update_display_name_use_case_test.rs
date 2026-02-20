// crates/profile/src/application/update_display_name/update_display_name_use_case_test.rs

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope, DomainEvent};
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::{DomainError, Result};

    use crate::application::use_cases::update_display_name::{UpdateDisplayNameCommand, UpdateDisplayNameUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{DisplayName, Handle, ProfileId}; // Ajout Handle et ProfileId
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
        let owner_id = AccountId::new(); // Contexte proprio
        let region = RegionCode::try_new("eu").unwrap();
        let initial_profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Old Name"),
            Handle::try_new("user123").unwrap(), // Username -> Handle
        )
            .build();

        let profile_id = initial_profile.id().clone(); // Pivot identité
        let use_case = setup(Some(initial_profile));
        let new_display_name = DisplayName::from_raw("New Name");

        let cmd = UpdateDisplayNameCommand {
            profile_id: profile_id.clone(),
            region: region.clone(),
            new_display_name: new_display_name.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.display_name(), &new_display_name);
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_display_name_fails_on_region_mismatch() {
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

        let cmd = UpdateDisplayNameCommand {
            profile_id,
            region: wrong_region,
            new_display_name: DisplayName::from_raw("Hacker Name"),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_update_display_name_idempotency() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let current_name = DisplayName::from_raw("Alice");

        let profile = Profile::builder(
            owner_id,
            region.clone(),
            current_name.clone(),
            Handle::try_new("alice").unwrap(),
        ).build();

        let profile_id = profile.id().clone();
        let use_case = setup(Some(profile));

        let cmd = UpdateDisplayNameCommand {
            profile_id,
            region,
            new_display_name: current_name,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.version(), 1);
    }

    #[tokio::test]
    async fn test_update_display_name_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = UpdateDisplayNameCommand {
            profile_id: ProfileId::new(),
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
            profile_id,
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
            profile_id,
            region,
            new_display_name: DisplayName::from_raw("Failing Name"),
        }).await;

        // Assert
        assert!(result.is_err());
    }
}
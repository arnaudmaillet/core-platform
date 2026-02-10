// crates/profile/src/application/update_location_label/update_location_label_use_case_test.rs

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope, DomainEvent};
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, LocationLabel, RegionCode};
    use shared_kernel::errors::{DomainError, Result};

    use crate::application::update_location_label::{
        UpdateLocationLabelCommand, UpdateLocationLabelUseCase,
    };
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{DisplayName, Handle, ProfileId}; // Ajout ProfileId et Handle
    use crate::domain::repositories::ProfileRepositoryStub;

    /// Helper pour configurer le Use Case avec ses dépendances
    fn setup(profile: Option<Profile>) -> UpdateLocationLabelUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        UpdateLocationLabelUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        )
    }

    #[tokio::test]
    async fn test_update_location_success() {
        // Arrange
        let owner_id = AccountId::new(); // Contexte proprio
        let region = RegionCode::try_new("eu").unwrap();
        let initial_profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(), // Username -> Handle
        )
            .build();

        let profile_id = initial_profile.id().clone(); // Pivot identité
        let use_case = setup(Some(initial_profile));
        let new_location = Some(LocationLabel::try_new("Paris, France").unwrap());

        let cmd = UpdateLocationLabelCommand {
            profile_id: profile_id.clone(),
            region: region.clone(),
            new_location: new_location.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.location_label(), new_location.as_ref());
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_location_fails_on_region_mismatch() {
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
        let new_location = Some(LocationLabel::try_new("Hacker Space").unwrap());

        let cmd = UpdateLocationLabelCommand {
            profile_id,
            region: wrong_region,
            new_location,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_remove_location_success() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        ).build();

        let profile_id = profile.id().clone();
        profile.update_location_label(&region, Some(LocationLabel::try_new("Tokyo").unwrap())).unwrap();

        let use_case = setup(Some(profile));

        let cmd = UpdateLocationLabelCommand {
            profile_id,
            region,
            new_location: None,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert!(updated.location_label().is_none());
        assert_eq!(updated.version(), 3);
    }

    #[tokio::test]
    async fn test_update_location_idempotency() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let location = Some(LocationLabel::try_new("Berlin").unwrap());

        let mut profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        ).build();

        let profile_id = profile.id().clone();
        profile.update_location_label(&region, location.clone()).unwrap();

        let use_case = setup(Some(profile));

        let cmd = UpdateLocationLabelCommand {
            profile_id,
            region,
            new_location: location,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_location_not_found() {
        // Arrange
        let use_case = setup(None);
        let cmd = UpdateLocationLabelCommand {
            profile_id: ProfileId::new(),
            region: RegionCode::try_new("eu").unwrap(),
            new_location: Some(LocationLabel::try_new("Mars").unwrap()),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_location_concurrency_conflict() {
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

        let use_case = UpdateLocationLabelUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        );

        let cmd = UpdateLocationLabelCommand {
            profile_id,
            region,
            new_location: Some(LocationLabel::try_new("New York").unwrap()),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_update_location_atomic_rollback_on_outbox_failure() {
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
                Err(DomainError::Internal("Outbox capacity reached".into()))
            }
            async fn find_pending(&self, _: i32) -> Result<Vec<EventEnvelope>> { Ok(vec![]) }
        }

        let use_case = UpdateLocationLabelUseCase::new(
            Arc::new(ProfileRepositoryStub {
                profile_to_return: Mutex::new(Some(profile)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        let cmd = UpdateLocationLabelCommand {
            profile_id,
            region,
            new_location: Some(LocationLabel::try_new("London").unwrap()),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_err());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{AccountId, LocationLabel, RegionCode, Username};
    use shared_kernel::errors::DomainError;
    use crate::application::update_location_label::{UpdateLocationLabelCommand, UpdateLocationLabelUseCase};
    use crate::domain::builders::ProfileBuilder;
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;
    use crate::utils::profile_repository_stub::{ProfileRepositoryStub, OutboxRepoStub, StubTxManager};

    fn setup(profile: Option<Profile>) -> UpdateLocationLabelUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        UpdateLocationLabelUseCase::new(
            repo,
            Arc::new(OutboxRepoStub),
            Arc::new(StubTxManager),
        )
    }

    #[tokio::test]
    async fn test_update_location_success() {
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
        // Utilisation du VO LocationLabel
        let new_location = Some(LocationLabel::try_new("Paris, France").unwrap());

        let cmd = UpdateLocationLabelCommand {
            account_id,
            region,
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
    async fn test_remove_location_success() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Alice"), Username::try_new("alice").unwrap()).build();

        // On initialise avec une localisation via le VO
        profile.update_location_label(Some(LocationLabel::try_new("Tokyo").unwrap()));

        let use_case = setup(Some(profile));

        let cmd = UpdateLocationLabelCommand {
            account_id,
            region,
            new_location: None, // Suppression
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
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let location = Some(LocationLabel::try_new("Berlin").unwrap());

        let mut profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Alice"), Username::try_new("alice").unwrap()).build();
        profile.update_location_label(location.clone());

        let use_case = setup(Some(profile));

        let cmd = UpdateLocationLabelCommand {
            account_id,
            region,
            new_location: location,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        // L'idempotence basée sur l'égalité du VO empêche l'incrément
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_location_not_found() {
        let use_case = setup(None);
        let cmd = UpdateLocationLabelCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
            new_location: Some(LocationLabel::try_new("Mars").unwrap()),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_location_concurrency_conflict() {
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Alice"), Username::try_new("alice").unwrap()).build();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch".into()
            })),
            ..Default::default()
        });

        let use_case = UpdateLocationLabelUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        let result = use_case.execute(UpdateLocationLabelCommand {
            account_id,
            region,
            new_location: Some(LocationLabel::try_new("New York").unwrap()),
        }).await;

        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_update_location_atomic_rollback_on_outbox_failure() {
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Alice"), Username::try_new("alice").unwrap()).build();

        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(&self, _: &mut dyn shared_kernel::domain::transaction::Transaction, _: &dyn shared_kernel::domain::events::DomainEvent) -> shared_kernel::errors::Result<()> {
                Err(DomainError::Internal("Outbox error".into()))
            }
        }

        let use_case = UpdateLocationLabelUseCase::new(
            Arc::new(ProfileRepositoryStub {
                profile_to_return: Mutex::new(Some(profile)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        let result = use_case.execute(UpdateLocationLabelCommand {
            account_id,
            region,
            new_location: Some(LocationLabel::try_new("London").unwrap()),
        }).await;

        assert!(result.is_err());
    }
}
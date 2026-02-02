#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode};
    use shared_kernel::errors::DomainError;
    use crate::application::update_display_name::{UpdateDisplayNameCommand, UpdateDisplayNameUseCase};
    use crate::domain::builders::ProfileBuilder;
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;
    use crate::utils::profile_repository_stub::{ProfileRepositoryStub, OutboxRepoStub, StubTxManager};

    fn setup(profile: Option<Profile>) -> UpdateDisplayNameUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        UpdateDisplayNameUseCase::new(
            repo,
            Arc::new(OutboxRepoStub),
            Arc::new(StubTxManager),
        )
    }

    #[tokio::test]
    async fn test_update_display_name_success() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let initial_profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Ancien Nom"),
            Username::try_new("user1").unwrap()
        ).build();

        let use_case = setup(Some(initial_profile));
        let new_name = DisplayName::from_raw("Nouveau Nom");

        let cmd = UpdateDisplayNameCommand {
            account_id,
            region,
            new_display_name: new_name.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.display_name().as_str(), "Nouveau Nom");
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_display_name_idempotency() {
        // Arrange : On essaie de mettre le même nom
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let name = DisplayName::from_raw("Alice");

        let profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            name.clone(),
            Username::try_new("alice").unwrap()
        ).build();

        let use_case = setup(Some(profile));

        let cmd = UpdateDisplayNameCommand {
            account_id,
            region,
            new_display_name: name,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let profile = result.unwrap();
        // L'idempotence métier (events.is_empty()) doit empêcher l'incrément de version
        assert_eq!(profile.version(), 1);
    }

    #[tokio::test]
    async fn test_update_display_name_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = UpdateDisplayNameCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
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
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Nom"), Username::try_new("user").unwrap()).build();

        // Simulation d'une collision de version au moment du save
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version conflict".into()
            })),
            ..Default::default()
        });

        let use_case = UpdateDisplayNameUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        // Act
        let result = use_case.execute(UpdateDisplayNameCommand {
            account_id,
            region,
            new_display_name: DisplayName::from_raw("New Name"),
        }).await;

        // Assert
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_update_display_name_outbox_failure() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(account_id.clone(), region.clone(), DisplayName::from_raw("Nom"), Username::try_new("user").unwrap()).build();

        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(&self, _: &mut dyn shared_kernel::domain::transaction::Transaction, _: &dyn shared_kernel::domain::events::DomainEvent) -> shared_kernel::errors::Result<()> {
                Err(DomainError::Internal("Outbox full".into()))
            }
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
            new_display_name: DisplayName::from_raw("Update"),
        }).await;

        // Assert
        // Si l'Outbox échoue, le Use Case doit remonter l'erreur (Rollback)
        assert!(result.is_err());
    }
}
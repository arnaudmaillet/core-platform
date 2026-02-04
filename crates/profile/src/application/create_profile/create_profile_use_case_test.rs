#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::create_profile::CreateProfileCommand;
    use crate::application::create_profile::create_profile_use_case::CreateProfileUseCase;
    use crate::domain::value_objects::DisplayName;
    use crate::utils::profile_repository_stub::{
        OutboxRepoStub, ProfileRepositoryStub, StubTxManager,
    };
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope};
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
    use shared_kernel::errors::{DomainError, Result};
    use std::sync::{Arc, Mutex};

    fn setup() -> CreateProfileUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(None),
            exists_return: Mutex::new(false),
            error_to_return: Mutex::new(None),
        });

        CreateProfileUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager))
    }

    #[tokio::test]
    async fn test_create_profile_success() {
        // Arrange
        let use_case = setup();
        let account_id = AccountId::new();
        let username = Username::try_new("john_doe").unwrap();

        let cmd = CreateProfileCommand {
            account_id: account_id.clone(),
            region: RegionCode::from_raw("eu"),
            display_name: DisplayName::from_raw("John"),
            username: username.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let profile = result.unwrap();
        assert_eq!(profile.account_id(), &account_id);
        assert_eq!(profile.username(), &username);
        assert_eq!(profile.version(), 1);
        assert_eq!(profile.post_count(), 0);
    }

    #[tokio::test]
    async fn test_create_profile_conflict_username() {
        // Arrange : On simule que le pseudo existe déjà
        let repo = Arc::new(ProfileRepositoryStub {
            exists_return: Mutex::new(true), // Simulation du doublon
            ..Default::default()
        });

        let use_case =
            CreateProfileUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        let cmd = CreateProfileCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
            display_name: DisplayName::from_raw("John"),
            username: Username::try_new("already_taken").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(
            matches!(result, Err(DomainError::AlreadyExists { field, .. }) if field == "username")
        );
    }

    #[tokio::test]
    async fn test_create_profile_repository_error() {
        // Arrange : Erreur DB lors de l'insertion
        let repo = Arc::new(ProfileRepositoryStub {
            error_to_return: Mutex::new(Some(DomainError::Internal("DB Error".into()))),
            ..Default::default()
        });

        let use_case =
            CreateProfileUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        let cmd = CreateProfileCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
            display_name: DisplayName::from_raw("John"),
            username: Username::try_new("johnny").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_profile_atomic_outbox_failure() {
        // Arrange : Le repo est ok, mais l'outbox échoue
        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(
                &self,
                _: &mut dyn shared_kernel::domain::transaction::Transaction,
                _: &dyn shared_kernel::domain::events::DomainEvent,
            ) -> Result<()> {
                Err(DomainError::Internal("Outbox disk full".into()))
            }

            async fn find_pending(&self, _limit: i32) -> Result<Vec<EventEnvelope>> {
                Ok(vec![])
            }
        }

        let use_case = CreateProfileUseCase::new(
            Arc::new(ProfileRepositoryStub::default()),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        let cmd = CreateProfileCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
            display_name: DisplayName::from_raw("John"),
            username: Username::try_new("johnny").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        // Si l'outbox échoue, le profil ne doit pas être considéré comme créé (rollback tx)
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_profile_race_condition_database_unique_violation() {
        // Arrange : On simule que exists_by_username dit "Non" (étape 1),
        // mais que le save échoue quand même car quelqu'un a été plus rapide (étape 2).
        let repo = Arc::new(ProfileRepositoryStub {
            exists_return: Mutex::new(false),
            error_to_return: Mutex::new(Some(DomainError::AlreadyExists {
                entity: "Profile",
                field: "username",
                value: "fast_user".to_string(),
            })),
            ..Default::default()
        });

        let use_case =
            CreateProfileUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        let cmd = CreateProfileCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
            display_name: DisplayName::from_raw("Fast"),
            username: Username::try_new("fast_user").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(
            matches!(result, Err(DomainError::AlreadyExists { field, .. }) if field == "username")
        );
    }
}

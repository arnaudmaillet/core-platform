// crates/profile/src/application/update_privacy/update_privacy_use_case_test.rs

#[cfg(test)]
mod tests {
    use crate::application::update_privacy::{UpdatePrivacyCommand, UpdatePrivacyUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;

    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope, DomainEvent};
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
    use shared_kernel::errors::{DomainError, Result};
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::domain::repositories::ProfileRepositoryStub;

    fn setup(profile: Option<Profile>) -> UpdatePrivacyUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        UpdatePrivacyUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        )
    }

    #[tokio::test]
    async fn test_update_privacy_to_private_success() {
        // Arrange : Profil public par défaut (is_private = false)
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let initial_profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
            .build();

        let use_case = setup(Some(initial_profile));
        let cmd = UpdatePrivacyCommand {
            account_id,
            region,
            is_private: true, // Passage en privé
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert!(updated.is_private());
        assert_eq!(updated.version(), 2); // Init(1) -> Update(2)
    }

    #[tokio::test]
    async fn test_update_privacy_fails_on_region_mismatch() {
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

        let cmd = UpdatePrivacyCommand {
            account_id,
            region: wrong_region,
            is_private: true,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert : Doit être bloqué par l'entité
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_update_privacy_idempotency() {
        // Arrange : Profil déjà privé
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
            .build();

        // On le passe en privé manuellement pour le setup (nécessite la région)
        profile.update_privacy(&region, true).unwrap(); // Version passe à 2

        let use_case = setup(Some(profile));

        let cmd = UpdatePrivacyCommand {
            account_id: account_id.clone(),
            region,
            is_private: true, // On redemande "privé"
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let profile_result = result.unwrap();
        // L'idempotence métier empêche l'incrément de version (reste à 2)
        assert_eq!(profile_result.version(), 2);
    }

    #[tokio::test]
    async fn test_update_privacy_profile_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = UpdatePrivacyCommand {
            account_id: AccountId::new(),
            region: RegionCode::try_new("eu").unwrap(),
            is_private: true,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_privacy_concurrency_conflict() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
            .build();

        // Stub simulant une erreur de version lors du save
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Modified by another session".into(),
            })),
            ..Default::default()
        });

        let use_case = UpdatePrivacyUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        );

        // Act
        let result = use_case
            .execute(UpdatePrivacyCommand {
                account_id,
                region,
                is_private: true,
            })
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_update_privacy_atomic_outbox_failure() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
            .build();

        struct FailingOutbox;
        #[async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(&self, _: &mut dyn shared_kernel::domain::transaction::Transaction, _: &dyn DomainEvent) -> Result<()> {
                Err(DomainError::Internal("Outbox failure".into()))
            }
            async fn find_pending(&self, _: i32) -> Result<Vec<EventEnvelope>> { Ok(vec![]) }
        }

        let use_case = UpdatePrivacyUseCase::new(
            Arc::new(ProfileRepositoryStub {
                profile_to_return: Mutex::new(Some(profile)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        // Act
        let result = use_case
            .execute(UpdatePrivacyCommand {
                account_id,
                region,
                is_private: true,
            })
            .await;

        // Assert
        assert!(result.is_err());
    }
}
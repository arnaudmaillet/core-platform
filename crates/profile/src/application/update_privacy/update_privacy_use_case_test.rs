// crates/profile/src/application/update_privacy/update_privacy_use_case_test.rs

#[cfg(test)]
mod tests {
    use crate::application::update_privacy::{UpdatePrivacyCommand, UpdatePrivacyUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{DisplayName, Handle, ProfileId}; // Ajout Handle et ProfileId

    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope, DomainEvent};
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
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
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let initial_profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(), // Username -> Handle
        )
            .build();

        let profile_id = initial_profile.id().clone(); // Pivot identité
        let use_case = setup(Some(initial_profile));

        let cmd = UpdatePrivacyCommand {
            profile_id: profile_id.clone(),
            region,
            is_private: true, // Passage en privé
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert!(updated.is_private());
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_privacy_fails_on_region_mismatch() {
        // Arrange
        let owner_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();
        let wrong_region = RegionCode::try_new("us").unwrap();

        let profile = Profile::builder(
            owner_id,
            actual_region,
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        ).build();

        let profile_id = profile.id().clone();
        let use_case = setup(Some(profile));

        let cmd = UpdatePrivacyCommand {
            profile_id,
            region: wrong_region,
            is_private: true,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_update_privacy_idempotency() {
        // Arrange : Profil déjà privé
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        )
            .build();

        let profile_id = profile.id().clone();
        profile.update_privacy(&region, true).unwrap(); // Version passe à 2

        let use_case = setup(Some(profile));

        let cmd = UpdatePrivacyCommand {
            profile_id,
            region,
            is_private: true, // On redemande "privé"
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let profile_result = result.unwrap();
        assert_eq!(profile_result.version(), 2);
    }

    #[tokio::test]
    async fn test_update_privacy_profile_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = UpdatePrivacyCommand {
            profile_id: ProfileId::new(),
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
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        )
            .build();

        let profile_id = profile.id().clone();

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
                profile_id,
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
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        )
            .build();

        let profile_id = profile.id().clone();

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
                profile_id,
                region,
                is_private: true,
            })
            .await;

        // Assert
        assert!(result.is_err());
    }
}
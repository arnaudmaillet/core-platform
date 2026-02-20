#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use crate::application::use_cases::increment_post_count::{
        IncrementPostCountCommand, IncrementPostCountUseCase,
    };
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{DisplayName, Handle, ProfileId}; // Ajout Handle et ProfileId
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope};
    use shared_kernel::domain::value_objects::{AccountId, PostId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::domain::repositories::ProfileRepositoryStub;

    fn setup(profile: Option<Profile>) -> IncrementPostCountUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        IncrementPostCountUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager))
    }

    #[tokio::test]
    async fn test_increment_post_count_success() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        let initial_profile = Profile::builder(
            owner_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        )
            .build();

        let profile_id = initial_profile.id().clone(); // On récupère l'ID généré par le builder
        assert_eq!(initial_profile.post_count(), 0);

        let use_case = setup(Some(initial_profile));
        let post_id = PostId::new();

        let cmd = IncrementPostCountCommand {
            profile_id: profile_id.clone(), // Le pivot est maintenant le profile_id
            region: region.clone(),
            post_id,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        assert_eq!(updated.post_count(), 1);
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_increment_multiple_times_logic() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            owner_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        )
            .build();

        let profile_id = profile.id().clone();

        for _ in 0..5 {
            profile.increment_post_count(&region, PostId::new()).unwrap();
        }

        let use_case = setup(Some(profile));
        let cmd = IncrementPostCountCommand {
            profile_id,
            region,
            post_id: PostId::new(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        let updated = result.unwrap();
        assert_eq!(updated.post_count(), 6);
        assert_eq!(updated.version(), 7);
    }

    #[tokio::test]
    async fn test_increment_fails_on_region_mismatch() {
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

        let cmd = IncrementPostCountCommand {
            profile_id,
            region: wrong_region,
            post_id: PostId::new(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_increment_post_count_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = IncrementPostCountCommand {
            profile_id: ProfileId::new(), // ID inexistant
            region: RegionCode::try_new("eu").unwrap(),
            post_id: PostId::new(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_increment_concurrency_conflict_triggers_retry() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        )
            .build();

        let profile_id = profile.id().clone();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch".into(),
            })),
            ..Default::default()
        });

        let use_case = IncrementPostCountUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager));

        // Act
        let result = use_case.execute(IncrementPostCountCommand {
            profile_id,
            region,
            post_id: PostId::new(),
        }).await;

        // Assert
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_increment_atomic_rollback_on_outbox_failure() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        )
            .build();

        let profile_id = profile.id().clone();

        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(&self, _: &mut dyn shared_kernel::domain::transaction::Transaction, _: &dyn shared_kernel::domain::events::DomainEvent) -> shared_kernel::errors::Result<()> {
                Err(DomainError::Internal("Outbox capacity reached".into()))
            }
            async fn find_pending(&self, _limit: i32) -> shared_kernel::errors::Result<Vec<EventEnvelope>> { Ok(vec![]) }
        }

        let use_case = IncrementPostCountUseCase::new(
            Arc::new(ProfileRepositoryStub {
                profile_to_return: Mutex::new(Some(profile)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        // Act
        let result = use_case.execute(IncrementPostCountCommand {
            profile_id,
            region,
            post_id: PostId::new(),
        }).await;

        // Assert
        assert!(result.is_err());
    }
}
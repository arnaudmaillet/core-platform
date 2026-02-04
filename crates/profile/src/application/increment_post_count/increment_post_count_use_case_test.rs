#[cfg(test)]
mod tests {
    use crate::application::increment_post_count::{
        IncrementPostCountCommand, IncrementPostCountUseCase,
    };
    use crate::domain::builders::ProfileBuilder;
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;
    use crate::utils::profile_repository_stub::{
        OutboxRepoStub, ProfileRepositoryStub, StubTxManager,
    };
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope};
    use shared_kernel::domain::value_objects::{AccountId, PostId, RegionCode, Username};
    use shared_kernel::errors::DomainError;
    use std::sync::{Arc, Mutex};

    fn setup(profile: Option<Profile>) -> IncrementPostCountUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        IncrementPostCountUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager))
    }

    #[tokio::test]
    async fn test_increment_post_count_success() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let initial_profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
        .build();

        // On vérifie que le compteur est à 0 au départ
        assert_eq!(initial_profile.post_count(), 0);

        let use_case = setup(Some(initial_profile));
        let post_id = PostId::new();

        let cmd = IncrementPostCountCommand {
            account_id,
            region,
            post_id,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        // Le compteur doit être à 1 et la version a augmenté (Init 1 -> Inc 2)
        assert_eq!(updated.post_count(), 1);
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_increment_multiple_times_logic() {
        // Arrange : Profil ayant déjà 5 posts
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
        .build();

        // Simuler 5 incréments (version passera à 6)
        for _ in 0..5 {
            profile.increment_post_count(PostId::new());
        }

        let use_case = setup(Some(profile));
        let cmd = IncrementPostCountCommand {
            account_id,
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
    async fn test_increment_post_count_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = IncrementPostCountCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
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
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
        .build();

        // On simule une erreur de conflit au save
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch".into(),
            })),
            ..Default::default()
        });

        let use_case =
            IncrementPostCountUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        // Act
        let result = use_case
            .execute(IncrementPostCountCommand {
                account_id,
                region,
                post_id: PostId::new(),
            })
            .await;

        // Assert
        // Le with_retry va tenter plusieurs fois avant de rendre les armes
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_increment_atomic_rollback_on_outbox_failure() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = ProfileBuilder::new(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
        .build();

        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(
                &self,
                _: &mut dyn shared_kernel::domain::transaction::Transaction,
                _: &dyn shared_kernel::domain::events::DomainEvent,
            ) -> shared_kernel::errors::Result<()> {
                Err(DomainError::Internal("Outbox capacity reached".into()))
            }

            async fn find_pending(&self, _limit: i32) -> shared_kernel::errors::Result<Vec<EventEnvelope>> {
                Ok(vec![])
            }
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
        let result = use_case
            .execute(IncrementPostCountCommand {
                account_id,
                region,
                post_id: PostId::new(),
            })
            .await;

        // Assert
        // Si l'Outbox crash, le compteur ne doit pas être considéré comme incrémenté en base
        assert!(result.is_err());
    }
}

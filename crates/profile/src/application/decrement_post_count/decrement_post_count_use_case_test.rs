#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use crate::application::decrement_post_count::{
        DecrementPostCountCommand, DecrementPostCountUseCase,
    };
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope};
    use shared_kernel::domain::value_objects::{AccountId, PostId, RegionCode, Username};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::repositories::OutboxRepoStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::domain::repositories::ProfileRepositoryStub;

    fn setup(profile: Option<Profile>) -> DecrementPostCountUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        DecrementPostCountUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager))
    }

    #[tokio::test]
    async fn test_decrement_post_count_success() {
        // Arrange : Profil ayant 1 post
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
        .build();
        profile.increment_post_count(PostId::new()); // version 2, count 1

        let use_case = setup(Some(profile));
        let post_id = PostId::new();

        let cmd = DecrementPostCountCommand {
            account_id,
            region,
            post_id,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.post_count(), 0);
        assert_eq!(updated.version(), 3);
    }

    #[tokio::test]
    async fn test_decrement_prevent_negative_count() {
        // Arrange : Profil ayant 0 post
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
        .build();

        let use_case = setup(Some(profile));

        let cmd = DecrementPostCountCommand {
            account_id,
            region,
            post_id: PostId::new(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        // Le compteur reste à 0, pas d'événement, donc pas de transaction (version reste 1)
        assert_eq!(updated.post_count(), 0);
        assert_eq!(updated.version(), 1);
    }

    #[tokio::test]
    async fn test_decrement_post_count_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = DecrementPostCountCommand {
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
    async fn test_decrement_concurrency_conflict() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
        .build();
        profile.increment_post_count(PostId::new());

        // On simule une erreur de version lors du save
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch".into(),
            })),
            ..Default::default()
        });

        let use_case =
            DecrementPostCountUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        // Act
        let result = use_case
            .execute(DecrementPostCountCommand {
                account_id,
                region,
                post_id: PostId::new(),
            })
            .await;

        // Assert
        // Le retry est déclenché par with_retry, mais finit par échouer si l'erreur persiste
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_decrement_outbox_failure_rollbacks_count() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
        .build();
        profile.increment_post_count(PostId::new());

        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(
                &self,
                _: &mut dyn shared_kernel::domain::transaction::Transaction,
                _: &dyn shared_kernel::domain::events::DomainEvent,
            ) -> shared_kernel::errors::Result<()> {
                Err(DomainError::Internal("Disk Full".into()))
            }

            async fn find_pending(&self, _limit: i32) -> shared_kernel::errors::Result<Vec<EventEnvelope>> {
                Ok(vec![])
            }
        }

        let use_case = DecrementPostCountUseCase::new(
            Arc::new(ProfileRepositoryStub {
                profile_to_return: Mutex::new(Some(profile)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        // Act
        let result = use_case
            .execute(DecrementPostCountCommand {
                account_id,
                region,
                post_id: PostId::new(),
            })
            .await;

        // Assert
        assert!(result.is_err());
    }
}

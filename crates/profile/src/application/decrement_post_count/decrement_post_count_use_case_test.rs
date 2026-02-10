#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use crate::application::decrement_post_count::{
        DecrementPostCountCommand, DecrementPostCountUseCase,
    };
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{DisplayName, Handle, ProfileId};
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope};
    use shared_kernel::domain::value_objects::{AccountId, PostId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::domain::repositories::ProfileRepositoryStub;

    fn setup(profile: Option<Profile>) -> DecrementPostCountUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        DecrementPostCountUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager))
    }

    #[tokio::test]
    async fn test_decrement_post_count_success() {
        // Arrange : Profil ayant 1 post
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let mut profile = Profile::builder(
            owner_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        )
            .build();

        // On doit passer la région ici aussi
        profile.increment_post_count(&region, PostId::new()).unwrap(); // version 2, count 1
        let use_case = setup(Some(profile.clone()));
        let post_id = PostId::new();

        let cmd = DecrementPostCountCommand {
            profile_id: profile.id().clone(),
            region: region.clone(),
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
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let profile = Profile::builder(
            owner_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        )
            .build();

        let use_case = setup(Some(profile.clone()));

        let cmd = DecrementPostCountCommand {
            profile_id: profile.id().clone(),
            region,
            post_id: PostId::new(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        // Le compteur reste à 0, decrement_post_count renvoie Ok(false),
        // pas d'événement, donc pas de transaction (version reste 1)
        assert_eq!(updated.post_count(), 0);
        assert_eq!(updated.version(), 1);
    }

    #[tokio::test]
    async fn test_decrement_post_count_not_found() {
        // Arrange
        let use_case = setup(None);

        let cmd = DecrementPostCountCommand {
            profile_id: ProfileId::new(),
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
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap(); // Utilise try_new
        let mut profile = Profile::builder(
            owner_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        )
            .build();

        // FIX: Ajout de la région ici
        profile.increment_post_count(&region, PostId::new()).unwrap();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile.clone())),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch".into(),
            })),
            ..Default::default()
        });

        let use_case = DecrementPostCountUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager));

        // Act
        let result = use_case
            .execute(DecrementPostCountCommand {
                profile_id: profile.id().clone(),
                region,
                post_id: PostId::new(),
            })
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_decrement_outbox_failure_rollbacks_count() {
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

        // FIX: Ajout de la région ici
        profile.increment_post_count(&region, PostId::new()).unwrap();

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
                profile_to_return: Mutex::new(Some(profile.clone())),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        // Act
        let result = use_case
            .execute(DecrementPostCountCommand {
                profile_id: profile.id().clone(),
                region,
                post_id: PostId::new(),
            })
            .await;

        // Assert
        assert!(result.is_err());
    }


    #[tokio::test]
    async fn test_decrement_fails_on_region_mismatch() {
        // Arrange
        let owner_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();
        let wrong_region = RegionCode::try_new("us").unwrap();

        let profile = Profile::builder(
            owner_id.clone(),
            actual_region,
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        ).build();

        let use_case = setup(Some(profile.clone()));

        let cmd = DecrementPostCountCommand {
            profile_id: profile.id().clone(),
            region: wrong_region, // Mismatch avec le profil
            post_id: PostId::new(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        // Doit renvoyer Forbidden car le check est dans l'entité
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}

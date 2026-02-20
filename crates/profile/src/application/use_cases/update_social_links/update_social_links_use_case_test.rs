// crates/profile/src/application/update_social_links/update_social_links_use_case_test.rs

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope, DomainEvent};
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url};
    use shared_kernel::errors::{DomainError, Result};

    use crate::application::use_cases::update_social_links::{
        UpdateSocialLinksCommand, UpdateSocialLinksUseCase,
    };
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{DisplayName, SocialLinks, Handle, ProfileId};
    use crate::domain::repositories::ProfileRepositoryStub;

    /// Helper pour configurer le Use Case avec ses dépendances
    fn setup(profile: Option<Profile>) -> UpdateSocialLinksUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        UpdateSocialLinksUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        )
    }

    #[tokio::test]
    async fn test_update_social_links_success() {
        // Arrange
        let owner_id = AccountId::new(); // Contexte proprio
        let region = RegionCode::try_new("eu").unwrap();
        let initial_profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(), // Username -> Handle
        ).build();

        let profile_id = initial_profile.id().clone(); // Pivot identité
        let use_case = setup(Some(initial_profile));

        let links = SocialLinks::new()
            .with_website(Some(Url::try_new("https://alice.dev").expect("FAIL: website URL")))
            .with_x(Some(Url::try_new("https://x.com/alice").expect("FAIL: X URL")))
            .with_linkedin(Some(Url::try_new("https://linkedin.com/in/alice").expect("FAIL: LinkedIn URL")))
            .try_build()
            .expect("FAIL: SocialLinks validation")
            .expect("FAIL: SocialLinks empty");

        let cmd = UpdateSocialLinksCommand {
            profile_id: profile_id.clone(),
            region: region.clone(),
            new_links: Some(links.clone()),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.social_links(), Some(&links));
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_social_links_fails_on_region_mismatch() {
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

        let cmd = UpdateSocialLinksCommand {
            profile_id,
            region: wrong_region,
            new_links: None,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_clear_social_links_success() {
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
        let links = SocialLinks::new()
            .with_website(Some(Url::try_new("https://same.com").unwrap()))
            .try_build()
            .expect("FAIL: SocialLinks validation")
            .expect("FAIL: SocialLinks returned None");

        profile.update_social_links(&region, Some(links.clone())).unwrap();

        let use_case = setup(Some(profile));

        // Act
        let result = use_case.execute(UpdateSocialLinksCommand {
            profile_id,
            region,
            new_links: None, // Clear
        }).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert!(updated.social_links().is_none());
        assert_eq!(updated.version(), 3);
    }

    #[tokio::test]
    async fn test_update_social_links_idempotency() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        let links = SocialLinks::new()
            .with_website(Some(Url::try_new("https://same.com").unwrap()))
            .try_build()
            .expect("FAIL: SocialLinks validation")
            .expect("FAIL: SocialLinks returned None");

        let mut profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        ).build();

        let profile_id = profile.id().clone();
        profile.update_social_links(&region, Some(links.clone())).unwrap();

        let use_case = setup(Some(profile));

        // Act
        let result = use_case.execute(UpdateSocialLinksCommand {
            profile_id,
            region,
            new_links: Some(links), // Même objet
        }).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_social_links_not_found() {
        // Arrange
        let use_case = setup(None);

        // Act
        let result = use_case.execute(UpdateSocialLinksCommand {
            profile_id: ProfileId::new(),
            region: RegionCode::try_new("eu").unwrap(),
            new_links: None,
        }).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_social_links_concurrency_conflict() {
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

        let use_case = UpdateSocialLinksUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        );

        // Act
        let result = use_case.execute(UpdateSocialLinksCommand {
            profile_id,
            region,
            new_links: Some(SocialLinks::default()),
        }).await;

        // Assert
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_update_social_links_atomic_rollback_on_outbox_failure() {
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
                Err(DomainError::Internal("Outbox capacity error".into()))
            }
            async fn find_pending(&self, _: i32) -> Result<Vec<EventEnvelope>> { Ok(vec![]) }
        }

        let use_case = UpdateSocialLinksUseCase::new(
            Arc::new(ProfileRepositoryStub {
                profile_to_return: Mutex::new(Some(profile)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        let links = SocialLinks::new()
            .with_website(Some(Url::try_new("https://fail.com").unwrap()))
            .try_build()
            .expect("FAIL: SocialLinks validation")
            .expect("FAIL: SocialLinks returned None");

        // Act
        let result = use_case.execute(UpdateSocialLinksCommand {
            profile_id,
            region,
            new_links: Some(links),
        }).await;

        // Assert
        assert!(result.is_err());
    }
}
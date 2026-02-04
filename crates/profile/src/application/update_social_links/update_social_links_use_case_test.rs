#[cfg(test)]
mod tests {
    use crate::application::update_social_links::{
        UpdateSocialLinksCommand, UpdateSocialLinksUseCase,
    };
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{DisplayName, SocialLinks};
    use crate::utils::profile_repository_stub::{
        OutboxRepoStub, ProfileRepositoryStub, StubTxManager,
    };
    use shared_kernel::domain::events::{AggregateRoot, EventEnvelope};
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url, Username};
    use shared_kernel::errors::DomainError;
    use std::sync::{Arc, Mutex};

    fn setup(profile: Option<Profile>) -> UpdateSocialLinksUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        UpdateSocialLinksUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager))
    }

    #[tokio::test]
    async fn test_update_social_links_success() {
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

        let links = SocialLinks::new()
            .with_website(Some(Url::try_from("https://alice.dev".to_string())
                .expect("FAIL: website URL malformed")))
            .with_x(Some(Url::try_from("https://x.com/alice".to_string())
                .expect("FAIL: X URL malformed")))
            .with_linkedin(Some(Url::try_from("https://linkedin.com/in/alice".to_string())
                .expect("FAIL: LinkedIn URL malformed")))
            .try_build()
            .expect("FAIL: SocialLinks validation error")
            .expect("FAIL: SocialLinks returned None (empty)");

        let cmd = UpdateSocialLinksCommand {
            account_id,
            region,
            new_links: Some(links.clone()),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.social_links(), Some(&links));
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_clear_social_links_success() {
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
            .build();

        let links = SocialLinks::new()
            .with_website(Some(Url::try_from("https://same.com".to_string())
                .expect("FAIL: website URL malformed")))
            .try_build()
            .expect("FAIL: SocialLinks validation error")
            .expect("FAIL: SocialLinks returned None");

        profile.update_social_links(Some(links.clone()));

        let use_case = setup(Some(profile));

        let result = use_case.execute(UpdateSocialLinksCommand {
            account_id,
            region,
            new_links: None,
        }).await;

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert!(updated.social_links().is_none());
        assert_eq!(updated.version(), 3);
    }

    #[tokio::test]
    async fn test_update_social_links_idempotency() {
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        let links = SocialLinks::new()
            .with_website(Some(Url::try_from("https://same.com".to_string())
                .expect("FAIL: website URL malformed")))
            .try_build()
            .expect("FAIL: SocialLinks validation error")
            .expect("FAIL: SocialLinks returned None");

        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        )
            .build();
        profile.update_social_links(Some(links.clone()));

        let use_case = setup(Some(profile));

        let result = use_case.execute(UpdateSocialLinksCommand {
            account_id,
            region,
            new_links: Some(links),
        }).await;

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.version(), 2);
    }

    #[tokio::test]
    async fn test_update_social_links_not_found() {
        let use_case = setup(None);
        let result = use_case.execute(UpdateSocialLinksCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
            new_links: None,
        }).await;

        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_social_links_concurrency_conflict() {
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        ).build();

        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch".into(),
            })),
            ..Default::default()
        });

        let use_case = UpdateSocialLinksUseCase::new(repo, Arc::new(OutboxRepoStub), Arc::new(StubTxManager));

        let result = use_case.execute(UpdateSocialLinksCommand {
            account_id,
            region,
            new_links: Some(SocialLinks::default()),
        }).await;

        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_update_social_links_atomic_rollback_on_outbox_failure() {
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        ).build();

        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(
                &self,
                _: &mut dyn shared_kernel::domain::transaction::Transaction,
                _: &dyn shared_kernel::domain::events::DomainEvent,
            ) -> shared_kernel::errors::Result<()> {
                Err(DomainError::Internal("Outbox error".into()))
            }
            async fn find_pending(&self, _limit: i32) -> shared_kernel::errors::Result<Vec<EventEnvelope>> {
                Ok(vec![])
            }
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
            .with_website(Some(Url::try_from("https://fail.com".to_string())
                .expect("FAIL: website URL malformed")))
            .try_build()
            .expect("FAIL: SocialLinks validation error")
            .expect("FAIL: SocialLinks returned None");

        let result = use_case.execute(UpdateSocialLinksCommand {
            account_id,
            region,
            new_links: Some(links),
        }).await;

        assert!(result.is_err());
    }
}
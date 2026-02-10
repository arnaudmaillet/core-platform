#[cfg(test)]
mod tests {
    use crate::application::get_profile_by_handle::{
        GetProfileByHandleCommand, GetProfileByHandleUseCase,
    };
    use crate::domain::entities::Profile;
    use crate::domain::repositories::{ProfileIdentityRepository, ProfileRepository, ProfileRepositoryStub};
    use crate::domain::value_objects::{DisplayName, Handle, ProfileId}; // Ajout Handle et ProfileId
    use shared_kernel::domain::repositories::{CacheRepository, CacheRepositoryStub};
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use std::sync::{Arc, Mutex};

    fn setup(
        profile: Option<Profile>,
        cached_json: Option<String>,
        fail_cache: bool,
    ) -> GetProfileByHandleUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            ..Default::default()
        });

        let mut cache_stub = CacheRepositoryStub::default();
        cache_stub.fail_all = fail_cache;

        if let Some(json) = cached_json {
            cache_stub
                .storage
                .lock()
                .unwrap()
                .insert("profile:h:eu:bob".to_string(), json);
        }

        GetProfileByHandleUseCase::new(repo, Arc::new(cache_stub))
    }

    #[tokio::test]
    async fn test_get_profile_cache_hit_success() {
        // Arrange
        let bob = Profile::builder(
            AccountId::new(),
            RegionCode::from_raw("eu"),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        )
            .build();
        let json = serde_json::to_string(&bob).unwrap();

        let use_case = setup(None, Some(json), false);

        let cmd = GetProfileByHandleCommand {
            handle: Handle::try_new("bob").unwrap(),
            region: RegionCode::from_raw("eu"),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap().handle().as_str(), "bob");
    }

    #[tokio::test]
    async fn test_get_profile_cache_miss_then_fill() {
        // 1. Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let handle = Handle::try_new("bob").unwrap();
        let cache_key = "profile:h:eu:bob"; // h pour handle

        let bob = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            handle.clone(),
        )
            .build();

        let repo_stub = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(bob)),
            ..Default::default()
        });

        let cache_stub = Arc::new(CacheRepositoryStub::default());
        let cache_for_assertion = Arc::clone(&cache_stub);
        let use_case = GetProfileByHandleUseCase::new(repo_stub, cache_stub);

        let cmd = GetProfileByHandleCommand {
            handle: handle.clone(),
            region,
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let cached_data = cache_for_assertion.get(cache_key).await.unwrap();

        assert!(cached_data.is_some());
        assert!(cached_data.unwrap().contains("bob"));
    }

    #[tokio::test]
    async fn test_get_profile_not_found_returns_error() {
        let use_case = setup(None, None, false);

        let cmd = GetProfileByHandleCommand {
            handle: Handle::try_new("ghost").unwrap(),
            region: RegionCode::from_raw("eu"),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_get_profile_resilience_on_redis_failure() {
        let bob = Profile::builder(
            AccountId::new(),
            RegionCode::from_raw("eu"),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        )
            .build();

        let use_case = setup(Some(bob), None, true);

        let cmd = GetProfileByHandleCommand {
            handle: Handle::try_new("bob").unwrap(),
            region: RegionCode::from_raw("eu"),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().handle().as_str(), "bob");
    }

    #[tokio::test]
    async fn test_get_profile_corrupted_cache_trigger_refresh() {
        let bob = Profile::builder(
            AccountId::new(),
            RegionCode::from_raw("eu"),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        )
            .build();
        let use_case = setup(Some(bob), Some("{{invalid_json}}".to_string()), false);

        let cmd = GetProfileByHandleCommand {
            handle: Handle::try_new("bob").unwrap(),
            region: RegionCode::from_raw("eu"),
        };

        let result = use_case.execute(cmd).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().handle().as_str(), "bob");
    }

    #[tokio::test]
    async fn test_get_profile_singleflight_actually_deduplicates_calls() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingRepo {
            call_count: Arc<AtomicUsize>,
            profile: Profile,
        }

        #[async_trait::async_trait]
        impl ProfileRepository for CountingRepo {
            // Les autres méthodes renvoient des données factices pour le test
            async fn assemble_full_profile(&self, _: &ProfileId, _: &RegionCode) -> shared_kernel::errors::Result<Option<Profile>> { Ok(None) }

            async fn resolve_profile_from_handle(
                &self,
                _: &Handle,
                _: &RegionCode,
            ) -> shared_kernel::errors::Result<Option<Profile>> {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                Ok(Some(self.profile.clone()))
            }
            async fn fetch_identity_only(&self, _: &ProfileId, _: &RegionCode) -> shared_kernel::errors::Result<Option<Profile>> { Ok(None) }
            async fn fetch_stats_only(&self, _: &ProfileId, _: &RegionCode) -> shared_kernel::errors::Result<Option<crate::domain::value_objects::ProfileStats>> { Ok(None) }
            async fn save_identity(&self, _: &Profile, _: Option<&Profile>, _: Option<&mut dyn shared_kernel::domain::transaction::Transaction>) -> shared_kernel::errors::Result<()> { Ok(()) }
            async fn exists_by_handle(&self, _: &Handle, _: &RegionCode) -> shared_kernel::errors::Result<bool> { Ok(true) }
            async fn delete_full_profile(&self, _: &ProfileId, _: &RegionCode) -> shared_kernel::errors::Result<()> { Ok(()) }
        }

        // Nécessaire car ProfileRepository peut dépendre de ProfileIdentityRepository
        #[async_trait::async_trait]
        impl ProfileIdentityRepository for CountingRepo {
            async fn save(&self, _: &Profile, _: Option<&mut dyn shared_kernel::domain::transaction::Transaction>) -> shared_kernel::errors::Result<()> { Ok(()) }
            async fn fetch(&self, _: &ProfileId, _: &RegionCode) -> shared_kernel::errors::Result<Option<Profile>> { Ok(Some(self.profile.clone())) }
            async fn fetch_by_handle(&self, _: &Handle, _: &RegionCode) -> shared_kernel::errors::Result<Option<Profile>> { Ok(Some(self.profile.clone())) }
            async fn fetch_all_by_owner(&self, owner_id: &AccountId) -> shared_kernel::errors::Result<Vec<Profile>> { Ok(vec![self.profile.clone()]) }
            async fn exists_by_handle(&self, _: &Handle, _: &RegionCode) -> shared_kernel::errors::Result<bool> { Ok(true) }
            async fn delete(&self, _: &ProfileId, _: &RegionCode) -> shared_kernel::errors::Result<()> { Ok(()) }
        }

        let counter = Arc::new(AtomicUsize::new(0));
        let bob = Profile::builder(
            AccountId::new(),
            RegionCode::from_raw("eu"),
            DisplayName::from_raw("Bob"),
            Handle::try_new("bob").unwrap(),
        )
            .build();

        let repo = Arc::new(CountingRepo {
            call_count: counter.clone(),
            profile: bob,
        });
        let cache = Arc::new(CacheRepositoryStub::default());
        let use_case = Arc::new(GetProfileByHandleUseCase::new(repo, cache));

        let mut handles = vec![];
        for _ in 0..10 {
            let uc = Arc::clone(&use_case);
            handles.push(tokio::spawn(async move {
                uc.execute(GetProfileByHandleCommand {
                    handle: Handle::try_new("bob").unwrap(),
                    region: RegionCode::from_raw("eu"),
                })
                    .await
            }));
        }

        for h in handles { h.await.unwrap().unwrap(); }

        assert_eq!(counter.load(Ordering::SeqCst), 1, "Le Singleflight n'a pas dédoublonné !");
    }
}